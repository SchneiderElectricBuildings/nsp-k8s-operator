use kube::api::DeleteParams;
use tracing::info;

use crate::clients::Clients;
use crate::config::snapshot;
use crate::controller::Ctx;
use crate::crd::v1alpha6::{EboServer, EboServerPhase, EboUpgradeStatus};
use crate::eboserver::handles::Handles;
use crate::eboserver::manager::EboServerManager;
use crate::eboserver::EboServerError;
use crate::events::definitions::OperatorEvent;
use crate::resources::{ensure_apply, job, pod, secret, util::gen_temp_password};
use crate::utils::ebo_version::ebo_version_ge;

use super::observe::Observed;
use super::types::{Decision, Effect, Transition};

pub async fn apply(
    ebo: &EboServer, h: &Handles, ctx: &Ctx, clients: &Clients<'_>, mgr: &mut EboServerManager, o: &Observed,
    d: &Decision<EboServerPhase>,
) -> Result<Option<Transition<EboServerPhase>>, EboServerError> {
    let cfg = snapshot(&ctx.config);
    let name = ebo.metadata.name.as_deref().unwrap();

    let mut queue: Vec<Effect> = d.effects.clone();
    let mut forced: Option<Transition<EboServerPhase>> = None;

    while let Some(eff) = queue.pop() {
        match eff {
            Effect::Publish(ev) => mgr.publish_event(ev).await?,

            Effect::ForcePhase(p) => {
                forced = Some(Transition::To(p.clone()));
            }

            Effect::UpdatePodStatus => {
                mgr.update_pod_status(o.pod.as_ref()).await?;
            }

            Effect::UpdateUpgradeJobStatus => {
                mgr.update_upgrade_job_status(o.upgrade_job.as_ref()).await?;
            }

            Effect::UpdateRestoreJobStatus => {
                mgr.update_restore_job_status(o.upgrade_job.as_ref()).await?;
            }

            Effect::UpdatePasswordResetJobStatus => {
                mgr.update_password_reset_job_status(o.password_reset_job.as_ref())
                    .await?;
            }

            Effect::UpdatePasswordResetMode => mgr.set_password_reset_mode_status().await?,

            Effect::UpdateResetTemporaryPassword => mgr.set_reset_temporary_password_status().await?,

            Effect::UpdatePasswordStatus(st) => {
                mgr.update_admin_password_status(st).await?;
            }

            Effect::SetBackupName(st) => {
                mgr.set_backup_name(st).await?;
            }

            Effect::SetUpgradeDisabled(st) => {
                mgr.set_upgrade_disabled(st).await?;
            }

            Effect::UpdateUpgradeStatus(st) => {
                mgr.update_upgrade_status(st).await?;
            }

            Effect::UpdateRestore => mgr.set_restore_status().await?,

            Effect::EnsureInitPasswordSecret => {
                let temporary_token = (Some(gen_temp_password(12)), Some("00:30:00".to_string()));

                ensure_apply(&h.secrets, &secret::gen_pass_secret(ebo, temporary_token, &cfg.ebo)).await?;
            }

            Effect::DeleteInitPasswordSecret => {
                h.secrets
                    .delete(&format!("{}-init-pass", name), &DeleteParams::default())
                    .await?;
            }

            Effect::EnsurePod => {
                ensure_apply(&h.pods, &pod::gen_pod(ebo, &cfg.ebo)).await?;
            }

            Effect::DeletePod => {
                h.pods.delete(name, &DeleteParams::default()).await?;
            }

            Effect::DeleteDataPvc => {
                // TODO: check if pvc exists?
                h.pvcs.delete(&format!("{name}-data"), &DeleteParams::default()).await?;
            }

            Effect::EnsureUpgradeJob { name: job_name } => {
                let job_data = job::gen_upgrade_job(ebo, job_name.clone(), &cfg.ebo);
                ensure_apply(&h.jobs, &job_data).await?;
            }

            Effect::EnsurePasswordResetJob { name: job_name } => {
                let job_data = job::gen_reset_password_job(ebo, job_name.clone(), &cfg.ebo);
                ensure_apply(&h.jobs, &job_data).await?;
            }

            Effect::EnsureRestoreJob {
                name: job_name,
                restore_name,
            } => {
                let job_data = job::gen_restore_job(ebo, job_name.clone(), restore_name, &cfg.ebo);
                ensure_apply(&h.jobs, &job_data).await?;
            }

            Effect::CheckAdminPassword => {
                // Extra safety: gate here too (even though decide gates it)
                if clients.mgmt_enabled && o.pod_ready {
                    if let Err(e) = mgr.check_admin_password_ebo(clients.mgmt_client).await {
                        tracing::warn!("EBO admin password check failed: {e}");
                    }
                }
            }

            Effect::TryStartBackup { name: backup_part_name } => {
                // Variant A: only try when MGMT enabled AND pod is ready.
                if !clients.mgmt_enabled || !o.pod_ready {
                    continue;
                }

                // ok to start backup, enough disk?
                let backup_ok = match mgr
                    .ok_to_perform_backup(clients.prometheus_client, cfg.ebo.backups)
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to fetch PVC stats: {}", e);
                        return Err(EboServerError::ServerNotFound); //TODO: look at error message
                    }
                };

                // ok to start backup
                if backup_ok {
                    let backup_name = format!("{}_{}", name, backup_part_name);
                    match mgr
                        .create_backup(clients.mgmt_client, cfg.ebo.backups, backup_name)
                        .await
                    {
                        Ok(()) => {
                            mgr.update_upgrade_status(EboUpgradeStatus::BackupOngoing).await?;
                        }
                        Err(e) => {
                            // Critical: do not set PerformingBackup on failure
                            tracing::warn!("Backup failed: {e}");
                        }
                    }
                } else {
                    tracing::debug!("Out of backup space");
                    queue.push(Effect::UpdateUpgradeStatus(EboUpgradeStatus::Ready));
                    queue.push(Effect::Publish(OperatorEvent::OutOfSpaceBackup));
                    queue.push(Effect::Publish(OperatorEvent::UpgradeFailed));
                    queue.push(Effect::SetUpgradeDisabled(true));
                    queue.push(Effect::ForcePhase(EboServerPhase::Ready));
                }
            }

            Effect::CheckBackupSucceeded { name: backup_part_name } => {
                if !clients.mgmt_enabled || !o.pod_ready {
                    continue;
                }

                let backup_name = format!("{}_{}", name, backup_part_name);
                // check that backup exists
                // TODO: EBO_VERSION_DEPENDENCY, if version is higher or same check; 7.1.2.44
                let lowest_accepted_version = "7.1.2.44";
                let running_version = mgr.get_running_ebo_version().await;
                if ebo_version_ge(running_version, lowest_accepted_version) {
                    match mgr.backup_performed(clients.mgmt_client, backup_name).await {
                        Ok(true) => {
                            queue.push(Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupSucceeded));
                            queue.push(Effect::SetBackupName("".to_owned()));
                        }
                        Ok(false) => {
                            // Critical: do not set BackupSucceeded on failure
                            queue.push(Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupFailed));
                            tracing::warn!("Backup not found");
                        }
                        Err(e) => {
                            // Critical: do not set BackupSucceeded on failure
                            queue.push(Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupFailed));
                            tracing::warn!("Backup failed: {e}");
                        }
                    };
                } else {
                    queue.push(Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupSucceeded));
                    queue.push(Effect::SetBackupName("".to_owned()));
                }
            }

            Effect::UpdateCrashLoopReported(v) => {
                mgr.set_crash_loop_reported(v).await?;
            }
        }
    }

    Ok(forced)
}

pub async fn apply_transition(
    mgr: &mut EboServerManager, transition: &Transition<EboServerPhase>,
) -> Result<(), EboServerError> {
    match transition {
        Transition::Stay => Ok(()),

        Transition::To(next) => {
            let prev = mgr.phase().as_ref().cloned();

            mgr.move_to(next.clone()).await?;

            info!(
                target: "controller::eboserver::phase",
                prev_phase = ?prev,
                next_phase = ?next,
                "phase transition"
            );

            Ok(())
        }
    }
}
