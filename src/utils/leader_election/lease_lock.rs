use super::LeaderError as Error;
use chrono::SecondsFormat;
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::jiff::Timestamp;
use kube::api::{PatchParams, PostParams};
use kube::ResourceExt;
use serde_json::json;
use std::time::Duration;
use tracing::instrument;

/// Represent a `LeaseLock` mechanism to try and acquire leadership status
pub struct LeaseLock {
    /// Parameters to describe the Lease
    params: LeaseLockParams,

    /// Handle to interact with Kubernetes Leases
    lease_api: kube::Api<Lease>,
}

/// Parameters to create a `LeaseLock` lock
#[derive(Debug)]
pub struct LeaseLockParams {
    /// Name of the Kubernetes Lease resource
    pub lease_name: String,

    /// Identity of the entity which wants to acquire the lock on the Lease
    pub holder_id: String,

    /// Lifetime of the lease
    pub lease_ttl: std::time::Duration,
}

/// Result of a `try_acquire_or_renew` call on a `LeaseLock`
#[derive(Default, Debug)]
pub struct LeaseLockResult {
    /// Bool to indicate whether leadership was acquired
    pub acquired_lease: bool,

    /// The latest `Lease` resource
    pub lease: Option<Lease>,
}

impl LeaseLock {
    /// Create a new `LeaseLock`
    #[must_use]
    pub fn new(client: kube::Client, namespace: &str, params: LeaseLockParams) -> Self {
        LeaseLock {
            params,
            lease_api: kube::Api::namespaced(client, namespace),
        }
    }

    /// Try to acquire the lock on the Kubernetes 'Lease' resource.
    ///
    /// Returns `LeaseLockResult` with information on the current state.
    #[instrument(skip(self))]
    pub async fn try_acquire_or_renew(&self) -> Result<LeaseLockResult, Error> {
        return match self.lease_api.get(&self.params.lease_name).await {
            Ok(l) => {
                if self.are_we_leading(&l)? {
                    let lease = self.renew_lease().await?;
                    tracing::debug!("successfully renewed lease {}", l.name_any());

                    Ok(LeaseLockResult {
                        acquired_lease: true,
                        lease: Some(lease),
                    })
                } else if self.has_lease_expired(&l)? {
                    let lease = self.acquire_lease(&l).await?;
                    tracing::info!("successfully acquired lease {}", lease.name_any());

                    Ok(LeaseLockResult {
                        acquired_lease: true,
                        lease: Some(lease),
                    })
                } else {
                    //tracing::info!(
                    //    "lease is held by {} and has not yet expired",
                    //    l.spec
                    //        .as_ref()
                    //        .ok_or(Error::TraverseLease {
                    //            key: "spec".to_string()
                    //        })?
                    //        .holder_identity
                    //        .as_ref()
                    //        .ok_or(Error::TraverseLease {
                    //            key: "spec.holderIdentity".to_string()
                    //        })?
                    //);

                    Ok(LeaseLockResult {
                        acquired_lease: false,
                        lease: None,
                    })
                }
            }
            Err(kube::Error::Api(api_err)) => {
                if api_err.code != 404 {
                    return Err(Error::ApiError { response: api_err });
                }

                let lease = self.create_lease().await?;
                tracing::info!("successfully acquired lease {}", lease.name_any());

                Ok(LeaseLockResult {
                    acquired_lease: true,
                    lease: Some(lease),
                })
            }
            Err(e) => Err(e.into()),
        };
    }

    /// Helper to determine if the current lease identity has leadership
    #[instrument(skip(self))]
    fn are_we_leading(&self, lease: &Lease) -> Result<bool, Error> {
        let holder_id = lease
            .spec
            .as_ref()
            .ok_or(Error::TraverseLease {
                key: "spec".to_string(),
            })?
            .holder_identity
            .as_ref()
            .ok_or(Error::TraverseLease {
                key: "spec.holderIdentity".to_string(),
            })?;

        Ok(holder_id.eq(&self.params.holder_id))
    }

    /// Helper to determine if the given lease has expired and can be acquired
    #[instrument(skip(self))]
    fn has_lease_expired(&self, lease: &Lease) -> Result<bool, Error> {
        let now = Timestamp::now();

        let spec = lease.spec.as_ref().ok_or(Error::TraverseLease {
            key: "spec".to_string(),
        })?;

        let last_renewed = spec
            .renew_time
            .as_ref()
            .ok_or(Error::TraverseLease {
                key: "spec.renewTime".to_string(),
            })?
            .0;

        let lease_duration = *spec.lease_duration_seconds.as_ref().ok_or(Error::TraverseLease {
            key: "spec.leaseDurationSeconds".to_string(),
        })?;

        let timeout = last_renewed + Duration::from_secs(lease_duration as u64);

        Ok(now > timeout)
    }

    /// Create a `Lease` resource in Kubernete
    #[instrument(skip(self))]
    async fn create_lease(&self) -> Result<Lease, Error> {
        let now: &str = &chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, false);

        let lease: Lease = serde_json::from_value(json!({
            "apiVersion": "coordination.k8s.io/v1",
            "kind": "Lease",
            "metadata": { "name": &self.params.lease_name },
            "spec": {
                "acquireTime": now,
                "renewTime": now,
                "holderIdentity": &self.params.holder_id,
                "leaseDurationSeconds": self.params.lease_ttl.as_secs(),
                "leaseTransitions": 0
            }
        }))?;

        self.lease_api
            .create(&PostParams::default(), &lease)
            .await
            .map_err(Error::CreateLease)
    }

    /// Acquire the `Lease` resource
    #[instrument(skip(self))]
    async fn acquire_lease(&self, lease: &Lease) -> Result<Lease, Error> {
        let now: &str = &chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, false);
        let transitions = &lease
            .spec
            .as_ref()
            .ok_or(Error::TraverseLease {
                key: "spec".to_string(),
            })?
            .lease_transitions
            .ok_or(Error::TraverseLease {
                key: "spec.leaseTransitions".to_string(),
            })?;

        let patch = json!({
            "apiVersion": "coordination.k8s.io/v1",
            "kind": "Lease",
            "metadata": { "name": &self.params.lease_name },
            "spec": {
                "acquireTime": now,
                "renewTime": now,
                "leaseTransitions": transitions + 1,
                "holderIdentity": &self.params.holder_id,
                "leaseDurationSeconds": self.params.lease_ttl.as_secs(),
            }
        });
        let patch = kube::api::Patch::Merge(&patch);

        self.lease_api
            .patch(&self.params.lease_name, &PatchParams::apply(&self.params.holder_id), &patch)
            .await
            .map_err(Error::AcquireLease)
    }

    /// Renew the `Lease` resource
    #[instrument(skip(self))]
    async fn renew_lease(&self) -> Result<Lease, Error> {
        let patch = json!({
            "apiVersion": "coordination.k8s.io/v1",
            "kind": "Lease",
            "metadata": { "name": &self.params.lease_name },
            "spec": {
                "renewTime": chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, false),
                "leaseDurationSeconds": self.params.lease_ttl.as_secs(),
            }
        });
        let patch = kube::api::Patch::Merge(&patch);

        self.lease_api
            .patch(&self.params.lease_name, &PatchParams::apply(&self.params.holder_id), &patch)
            .await
            .map_err(Error::RenewLease)
    }

    /// Release the lock if we hold it
    #[instrument(skip(self))]
    pub async fn step_down(&self) -> Result<(), Error> {
        let lease = self
            .lease_api
            .get(&self.params.lease_name)
            .await
            .map_err(Error::GetLease)?;

        if !self.are_we_leading(&lease)? {
            let leader = lease
                .spec
                .ok_or(Error::TraverseLease {
                    key: "spec".to_string(),
                })?
                .holder_identity
                .ok_or(Error::TraverseLease {
                    key: "spec.holderIdentity".to_string(),
                })?;
            return Err(Error::ReleaseLockWhenNotLeading { leader });
        }

        let now: &str = &chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, false);
        let patch = json!({
            "apiVersion": "coordination.k8s.io/v1",
            "kind": "Lease",
            "metadata": { "name": &self.params.lease_name },
            "spec": {
                "acquireTime": now,
                "renewTime": now,
                "leaseDurationSeconds": 1,
                "holderIdentity": ""
            }
        });
        let patch = kube::api::Patch::Merge(&patch);

        self.lease_api
            .patch(&self.params.lease_name, &PatchParams::apply(&self.params.holder_id), &patch)
            .await
            .map_err(Error::ReleaseLease)?;

        tracing::info!("successfully released lease {}", lease.name_any());

        Ok(())
    }
}
