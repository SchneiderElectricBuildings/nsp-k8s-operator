use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod, Secret};

use crate::clients::Clients;
use crate::config::snapshot;
use crate::controller::Ctx;
use crate::crd::v1alpha6::{EboAdminPasswordStatus, EboServer, EboUpgradeStatus};
use crate::eboserver::handles::Handles;
use crate::eboserver::manager::{
    has_job_succeeded, has_pod_stopped, is_crash_looping, is_pod_ready, is_pod_running, is_pod_terminating,
    total_restart_count, EboServerManager,
};
use crate::eboserver::reconcile::EboServerExt;
use crate::eboserver::EboServerError;
use crate::resources::util::calc_pod_image;
use crate::resources::{detect_drift, pod, pvc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupOngoing {
    Disabled,
    Unknown,
    Known(bool),
}

#[derive(Debug, Clone)]
pub struct Observed {
    pub name: String,
    pub deployed_by: String,
    pub top_server: bool,
    pub password_reset_mode: String,
    pub reset_temporary_password: String,

    pub pod: Option<Pod>,
    pub init_pass_secret: Option<Secret>,
    pub data_pvc: Option<PersistentVolumeClaim>,
    pub upgrade_job: Option<Job>,
    pub password_reset_job: Option<Job>,
    pub last_password_reset_mode: String,
    pub last_reset_temporary_password: String,

    pub pod_running: bool,
    pub pod_ready: bool,
    pub crash_loop: bool,
    pub restart_count: i32,
    pub crash_loop_reported: bool,
    pub pod_terminating: bool,
    pub pod_stopped: bool,

    pub drift_detected: bool,
    pub image_changed: bool,
    pub pvc_needs_resize: bool,

    pub upgrade_job_succeeded: bool,
    pub password_reset_job_succeeded: bool,

    pub restore: String,
    pub last_restore: String,
    pub restore_name: String,
    pub restore_job: Option<Job>,
    pub restore_job_succeeded: bool,

    pub admin_pwd_status: EboAdminPasswordStatus,

    pub mgmt_enabled: bool,

    pub backup_name: String,
    pub backup_ongoing: BackupOngoing,
    pub upgrade_status: EboUpgradeStatus,
    pub upgrade_disabled: bool,
}

impl Observed {
    pub fn pod_exists(&self) -> bool {
        self.pod.is_some()
    }
    pub fn data_pvc_exists(&self) -> bool {
        self.data_pvc.is_some()
    }
    pub fn secret_exists(&self) -> bool {
        self.init_pass_secret.is_some()
    }
    pub fn upgrade_job_exists(&self) -> bool {
        self.upgrade_job.is_some()
    }
    pub fn restore_job_exists(&self) -> bool {
        self.restore_job.is_some()
    }
    pub fn password_reset_job_exists(&self) -> bool {
        self.password_reset_job.is_some()
    }
}

pub async fn observe(
    ebo: &EboServer, h: &Handles, ctx: &Ctx, clients: &Clients<'_>, mgr: &mut EboServerManager,
) -> Result<Observed, EboServerError> {
    let cfg = snapshot(&ctx.config);
    let name = ebo.metadata.name.as_deref().unwrap().to_string();

    let deployed_by = ebo.get_deployed_by();
    let top_server = ebo.spec.top_server.unwrap_or(false);
    let password_reset_mode = ebo.spec.password_reset_mode.clone().unwrap_or_default();
    let reset_temporary_password = ebo.spec.reset_temporary_password.clone().unwrap_or_default();

    let pod_obj = h.pods.get_opt(&name).await?;
    let secret = h.secrets.get_opt(&format!("{name}-init-pass")).await?;
    let data_pvc = h.pvcs.get_opt(&format!("{name}-data")).await?;

    let backup_name = mgr.get_backup_name();
    let restore = ebo.spec.restore.clone().unwrap_or_default();
    let restore_name = ebo.spec.restore_name.clone().unwrap_or_default();

    let upgrade_job_name = format!("{name}-prepare-upgrade");
    let upgrade_job = h.jobs.get_opt(&upgrade_job_name).await?;
    let password_reset_job_name = format!("{name}-password-reset");
    let password_reset_job = h.jobs.get_opt(&password_reset_job_name).await?;
    let restore_job_name = format!("{name}-restore");
    let restore_job = h.jobs.get_opt(&restore_job_name).await?;

    // Derived pod booleans
    let pod_running = pod_obj.as_ref().is_some_and(is_pod_running);
    let pod_ready = pod_obj.as_ref().is_some_and(is_pod_ready);
    let crash_loop = pod_obj.as_ref().is_some_and(is_crash_looping);
    let restart_count = pod_obj.as_ref().map(total_restart_count).unwrap_or(0);
    let crash_loop_reported = mgr.get_crash_loop_reported();

    let pod_terminating = pod_obj.as_ref().is_some_and(is_pod_terminating);
    let pod_stopped = pod_obj.as_ref().is_some_and(has_pod_stopped);

    // Drift checks (only meaningful if pod exists)
    let drift_detected = pod_obj
        .as_ref()
        .is_some_and(|p| detect_drift(p, &pod::gen_pod(ebo, &cfg.ebo)));

    // Image change check: only meaningful if we have a current image recorded
    let image_changed = if pod_obj.is_some() && mgr.pod_image().is_some() {
        let current = mgr.pod_image().unwrap().split('/').next_back().unwrap().to_owned();
        let expected = calc_pod_image(ebo, &cfg.ebo.registry)
            .split('/')
            .next_back()
            .unwrap()
            .to_owned();
        current != expected
    } else {
        false
    };

    let pvc_needs_resize = data_pvc.as_ref().is_some_and(pvc::pvc_needs_update);
    let upgrade_job_succeeded = upgrade_job.as_ref().is_some_and(has_job_succeeded);
    let password_reset_job_succeeded = password_reset_job.as_ref().is_some_and(has_job_succeeded);
    let restore_job_succeeded = restore_job.as_ref().is_some_and(has_job_succeeded);

    let backup_ongoing = if !clients.mgmt_enabled {
        BackupOngoing::Disabled
    } else if !pod_ready {
        BackupOngoing::Known(false)
    } else if mgr.get_upgrade_status() != EboUpgradeStatus::Ready {
        match mgr.backup_status(clients.mgmt_client).await {
            Ok(v) => BackupOngoing::Known(v),
            Err(e) => {
                tracing::warn!("backup_status failed (treating as unknown): {e}");
                BackupOngoing::Unknown
            }
        }
    } else {
        BackupOngoing::Known(false)
    };

    Ok(Observed {
        name,
        deployed_by,
        top_server,
        password_reset_mode,
        reset_temporary_password,

        pod: pod_obj,
        init_pass_secret: secret,
        data_pvc,
        upgrade_job,
        password_reset_job,
        last_password_reset_mode: mgr.get_password_reset_mode_status(),
        last_reset_temporary_password: mgr.get_reset_temporary_password_status(),

        pod_running,
        pod_ready,
        crash_loop,
        restart_count,
        crash_loop_reported,
        pod_terminating,
        pod_stopped,

        drift_detected,
        image_changed,
        pvc_needs_resize,
        backup_name,
        upgrade_job_succeeded,

        restore,
        restore_name,
        last_restore: mgr.get_restore_status(),
        restore_job,
        restore_job_succeeded,

        password_reset_job_succeeded,

        admin_pwd_status: mgr.get_admin_password_status(),
        upgrade_status: mgr.get_upgrade_status(),
        mgmt_enabled: clients.mgmt_enabled,

        backup_ongoing,
        upgrade_disabled: mgr.get_upgrade_disabled(),
    })
}
