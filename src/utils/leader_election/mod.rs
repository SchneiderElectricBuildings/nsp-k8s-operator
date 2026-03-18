pub mod error;
pub mod lease_lock;

pub use error::LeaderError;
pub use lease_lock::{LeaseLock, LeaseLockParams, LeaseLockResult};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

pub fn spawn_leader_manager(client: kube::Client, ns: String, me: String, is_leader: Arc<AtomicBool>) {
    tokio::spawn(async move {
        let lock = LeaseLock::new(
            client.clone(),
            &ns,
            LeaseLockParams {
                holder_id: me.clone(),
                lease_name: "eboserver-operator-leader".into(),
                lease_ttl: Duration::from_secs(30),
            },
        );

        let mut tick = interval(Duration::from_secs(10));

        loop {
            tick.tick().await;

            // ---- per-tick span ----
            let tick_span = tracing::info_span!(
                parent: None,
                "leader_election_tick",
                holder_id = %me,
                namespace = %ns
            );
            let _enter = tick_span.enter();

            match lock.try_acquire_or_renew().await {
                Ok(res) => {
                    let leader_now = res.acquired_lease;
                    let prev = is_leader.swap(leader_now, Ordering::Relaxed);

                    if leader_now && !prev {
                        info!("Became leader");
                    } else if !leader_now && prev {
                        info!("Lost leadership");
                    } else {
                        tracing::debug!(leader = leader_now, "Leadership status unchanged");
                    }
                }
                Err(e) => {
                    is_leader.store(false, Ordering::Relaxed);
                    error!(error = %e, "Leader election error");
                }
            }
        }
    });
}
