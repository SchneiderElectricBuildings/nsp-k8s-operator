// hashring_manager.rs
use k8s_openapi::jiff::Timestamp;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use consistent_hash_ring::{Ring, RingBuilder};
use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, OwnerReference};
use kube::api::{ListParams, Patch, PatchParams};
use kube::{Api, Client};
use tokio::task::JoinHandle;
use tracing::{error, info, instrument};

/// Thread-safe handle to the current ring.
#[derive(Clone)]
pub struct RingHandle {
    inner: Arc<RwLock<Ring<String>>>,
}
impl RingHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// RF = 1 (single owner): am I the owner of `key`?
    pub fn is_owner(&self, me: &str, key: &str) -> bool {
        let ring = self.inner.read().unwrap();
        ring.try_get(key).is_some_and(|o| o == me)
    }
    /// RF > 1: first `rf` owners for `key`.
    pub fn owners(&self, key: &str, rf: usize) -> Vec<String> {
        let ring = self.inner.read().unwrap();
        ring.replicas(key).take(rf).cloned().collect()
    }
    fn set(&self, new_ring: Ring<String>) {
        *self.inner.write().unwrap() = new_ring;
    }
}

impl Default for RingHandle {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RingBuilder::default().build())),
        }
    }
}

/// Heartbeat a per-pod Lease with a **fixed label key/value** (no envs).
#[allow(clippy::too_many_arguments)]
pub fn spawn_membership_heartbeat(
    client: Client,
    ns: String,
    me: String,
    lease_name_prefix: String, // e.g. "serverctl-member-"
    lease_ttl: Duration,       // e.g. 30s
    renew_period: Duration,    // e.g. 10s (must be < lease_ttl)
    label_key: String,         // e.g. "app"
    label_value: String,       // e.g. "serverctl-member"
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let leases: Api<Lease> = Api::namespaced(client.clone(), &ns);
        let pods: Api<Pod> = Api::namespaced(client.clone(), &ns);
        let name = format!("{lease_name_prefix}{me}");
        let pp = PatchParams::apply("hashring-manager").force();
        let ttl_i32 = lease_ttl.as_secs().clamp(1, i64::MAX as u64) as i32;

        // Look up our Pod once to build OwnerReference
        let pod = match pods.get(&me).await {
            Ok(p) => p,
            Err(e) => {
                error!("cannot fetch Pod {}: {}", me, e);
                return;
            }
        };
        let Some(pod_uid) = pod.metadata.uid.clone() else {
            error!("Pod {} has no UID; cannot set ownerReference", me);
            return;
        };
        let owner = OwnerReference {
            api_version: "v1".into(),
            kind: "Pod".into(),
            name: me.clone(),
            uid: pod_uid,
            controller: Some(false),
            block_owner_deletion: Some(false),
        };

        loop {
            let tick_span = tracing::info_span!(
                parent: None,
                "membership_heartbeat_tick",
                holder_id = %me,
                namespace = %ns
            );
            let _enter = tick_span.enter();

            let mut labels = BTreeMap::new();
            labels.insert(label_key.clone(), label_value.clone());

            let lease = Lease {
                metadata: kube::core::ObjectMeta {
                    name: Some(name.clone()),
                    namespace: Some(ns.clone()),
                    labels: Some(labels),
                    owner_references: Some(vec![owner.clone()]), // GC hook
                    ..Default::default()
                },
                spec: Some(LeaseSpec {
                    holder_identity: Some(me.clone()),
                    lease_duration_seconds: Some(ttl_i32),
                    renew_time: Some(MicroTime(Timestamp::now())),
                    acquire_time: None,
                    lease_transitions: None,
                    ..LeaseSpec::default()
                }),
            };

            if let Err(e) = leases.patch(&name, &pp, &Patch::Apply(&lease)).await {
                error!("membership lease apply failed: {e}");
            }
            tokio::time::sleep(renew_period).await;
        }
    })
}

/// Watch Leases with the same **label key/value** and rebuild the ring.
#[allow(clippy::too_many_arguments)]
pub fn spawn_ring_manager(
    client: Client,
    ns: String,
    handle: RingHandle,
    label_key: String,
    label_value: String,
    vnodes: usize,           // e.g. 128
    poll_interval: Duration, // e.g. 5s
    ttl_skew: Duration,      // e.g. 5s
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let leases: Api<Lease> = Api::namespaced(client, &ns);
        let selector = format!("{label_key}={label_value}");
        let lp = ListParams::default().labels(&selector);

        // Track the last seen member list (sorted, deduped)
        let mut prev_members: Vec<String> = Vec::new();

        loop {
            let tick_span = tracing::info_span!(
                parent: None,
                "membership_heartbeat_tick",
                namespace = %ns
            );
            let _enter = tick_span.enter();

            match leases.list(&lp).await {
                Ok(list) => {
                    let now = Timestamp::now();
                    let mut members: Vec<String> = list
                        .items
                        .into_iter()
                        .filter_map(|l| l.spec)
                        .filter(|s| lease_alive(s, now, ttl_skew))
                        .filter_map(|s| s.holder_identity)
                        .collect();

                    members.sort();
                    members.dedup();

                    if members != prev_members {
                        let mut b = RingBuilder::default().vnodes(vnodes);
                        for id in &members {
                            b = b.node(id.clone());
                        }
                        handle.set(b.build());
                        // log only on change
                        info!(%selector, members = ?members, "hash ring changed; rebuilt");
                        prev_members = members;
                    } else {
                        // optional: lower-verbosity heartbeat
                        tracing::debug!(%selector, "hash ring unchanged");
                    }
                }
                Err(e) => {
                    // keep noisy errors visible
                    error!("ring list error: {e}");
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    })
}

#[instrument(skip(spec, now, skew))]
fn lease_alive(spec: &LeaseSpec, now: Timestamp, skew: Duration) -> bool {
    match (spec.renew_time.as_ref(), spec.lease_duration_seconds) {
        (Some(rt), Some(ttl)) => {
            let timeout = rt.0 + Duration::from_secs(ttl.max(0) as u64) + skew;
            now <= timeout
        }
        _ => true, // missing timing → treat as alive
    }
}
