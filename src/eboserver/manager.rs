use std::sync::Arc;

use crate::controller::{Ctx, OPERATOR_NAME};
use crate::crd::v1alpha6::{
    EboAdminPasswordStatus, EboLicenseStatus, EboServer, EboServerPhase, EboServerStatus, EboUpgradeStatus,
    EboWafStatus,
};
use crate::eboserver::EboServerError;
use chrono::Utc;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{ObjectReference, PersistentVolumeClaim, Pod};
use kube::api::{Patch, PatchParams};
use kube::runtime::events::{Event, EventType, Recorder};
use kube::{Api, Client, Error as kubeError, Resource, ResourceExt};
use tracing::instrument;

use crate::clients::{mgmt::MGMTClient, prometheus::PrometheusClient};
use crate::events::definitions::*;
use crate::events::Level;

pub struct EboServerManager {
    name: String,
    ns: String,
    client: Client,
    recorder: Recorder,
    object_ref: ObjectReference,
    status: EboServerStatus,
    pub id: String,
}

impl EboServerManager {
    pub fn new(ebo_server: &EboServer, ctx: Arc<Ctx>) -> Self {
        Self {
            name: ebo_server.metadata.name.clone().unwrap(),
            ns: ebo_server.metadata.namespace.clone().unwrap(),
            client: ctx.client.clone(),
            recorder: Recorder::new(ctx.client.clone(), ctx.reporter.clone()),
            object_ref: ebo_server.object_ref(&()),
            status: ebo_server.status.clone().unwrap_or_default(),
            id: ebo_server
                .labels()
                .get("systemId")
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        }
    }

    pub fn phase(&self) -> &Option<EboServerPhase> {
        &self.status.phase
    }

    pub fn pod_image(&self) -> Option<&String> {
        self.status.pod_image.as_ref()
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, phase = ?phase))]
    pub async fn move_to(&mut self, phase: EboServerPhase) -> Result<(), EboServerError> {
        self.status.phase = Some(phase);
        self.apply_status().await
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, event = %kind.kind()))]
    pub async fn publish_event(&self, kind: OperatorEvent) -> Result<(), kubeError> {
        let event_type = match kind.level() {
            Level::Info => EventType::Normal,
            Level::Warning | Level::Error => EventType::Warning,
        };
        // 1) K8s Event
        if kind.send_to_k8s() {
            self.recorder
                .publish(
                    &Event {
                        type_: event_type,
                        reason: kind.kind().to_string(),
                        note: Some(kind.message().to_owned()),
                        action: "PhaseUpdated".to_owned(),
                        secondary: None,
                    },
                    &self.object_ref.clone(),
                )
                .await?;
        }
        Ok(())
    }

    #[instrument(skip(self,pod), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn update_pod_status(&mut self, pod: Option<&Pod>) -> Result<(), EboServerError> {
        self.status.pod_phase = None;
        if let Some(pod_status) = pod.and_then(|pod| pod.status.as_ref()) {
            self.status.pod_phase.clone_from(&pod_status.phase);
            if let Some(css) = &pod_status.container_statuses {
                css.iter()
                    .filter(|cs| cs.name == "nsp-servers")
                    .for_each(|cs| self.status.pod_image = Some(cs.image.clone()));
            }
        }
        self.apply_status().await
    }

    #[instrument(skip(self, upgrade_job), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn update_upgrade_job_status(&mut self, upgrade_job: Option<&Job>) -> Result<(), EboServerError> {
        self.status.upgrade_job_status = None;
        if let Some(job_status) = upgrade_job.and_then(|job| job.status.as_ref()) {
            if job_status.succeeded.is_some_and(|s| s > 0) {
                self.status.upgrade_job_status = Some("Succeeded".to_owned());
            } else {
                self.status.upgrade_job_status = Some("Running".to_owned());
            }
        }
        self.apply_status().await
    }

    #[instrument(skip(self, restore_job), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn update_restore_job_status(&mut self, restore_job: Option<&Job>) -> Result<(), EboServerError> {
        self.status.restore_job_status = None;
        if let Some(job_status) = restore_job.and_then(|job| job.status.as_ref()) {
            if job_status.succeeded.is_some_and(|s| s > 0) {
                self.status.restore_job_status = Some("Succeeded".to_owned());
            } else {
                self.status.restore_job_status = Some("Running".to_owned());
            }
        }
        self.apply_status().await
    }

    #[instrument(skip(self, password_reset_job), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn update_password_reset_job_status(
        &mut self, password_reset_job: Option<&Job>,
    ) -> Result<(), EboServerError> {
        self.status.password_reset_job_status = None;
        if let Some(job_status) = password_reset_job.and_then(|job| job.status.as_ref()) {
            if job_status.succeeded.is_some_and(|s| s > 0) {
                self.status.password_reset_job_status = Some("Succeeded".to_owned());
            } else {
                self.status.password_reset_job_status = Some("Running".to_owned());
            }
        }
        self.apply_status().await
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, status = ?target_status))]
    pub async fn update_waf_status(&mut self, target_status: EboWafStatus) -> Result<(), EboServerError> {
        self.status.waf_status = Some(target_status);
        self.apply_status().await
    }

    pub fn get_waf_status(&self) -> EboWafStatus {
        self.status.waf_status.clone().unwrap_or_default()
    }

    pub fn get_registration_status(&self) -> bool {
        self.status.registered.unwrap_or_default()
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, registered = %registered))]
    pub async fn set_registration_status(&mut self, registered: bool) -> Result<(), EboServerError> {
        self.status.registered = Some(registered);
        self.apply_status().await
    }

    pub fn get_top_server_status(&self) -> bool {
        self.status.is_top_server.unwrap_or_default()
    }

    pub async fn set_top_server_status(&mut self, is_top_server: bool) -> Result<(), EboServerError> {
        self.status.is_top_server = Some(is_top_server);
        self.apply_status().await
    }

    pub fn get_license_status(&self) -> EboLicenseStatus {
        self.status.license_status.clone().unwrap_or_default()
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, status = ?target_status))]
    pub async fn update_license_status(&mut self, target_status: EboLicenseStatus) -> Result<(), EboServerError> {
        self.status.license_status = Some(target_status);
        self.apply_status().await
    }

    pub fn get_admin_password_status(&self) -> EboAdminPasswordStatus {
        self.status.admin_password_status.unwrap_or_default()
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, status = ?target_status))]
    pub async fn update_admin_password_status(
        &mut self, target_status: EboAdminPasswordStatus,
    ) -> Result<(), EboServerError> {
        self.status.admin_password_status = Some(target_status);
        self.apply_status().await
    }

    pub fn get_backup_name(&self) -> String {
        self.status.backup_name.clone().unwrap_or_default()
    }

    pub async fn set_backup_name(&mut self, backup_name: String) -> Result<(), EboServerError> {
        self.status.backup_name = Some(backup_name);
        self.apply_status().await
    }

    pub fn get_restore_status(&self) -> String {
        self.status.last_restore.clone().unwrap_or_default()
    }

    pub async fn set_restore_status(&mut self) -> Result<(), EboServerError> {
        let date = Utc::now().fixed_offset();
        self.status.last_restore = Some(date.to_rfc3339());
        self.apply_status().await
    }

    pub fn get_upgrade_disabled(&self) -> bool {
        self.status.upgrade_disabled.unwrap_or_default()
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, upgrade_disabled = %upgrade_disabled))]
    pub async fn set_upgrade_disabled(&mut self, upgrade_disabled: bool) -> Result<(), EboServerError> {
        self.status.upgrade_disabled = Some(upgrade_disabled);
        self.apply_status().await
    }

    pub fn get_upgrade_status(&self) -> EboUpgradeStatus {
        self.status.upgrade_status.unwrap_or_default()
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, status = ?target_status))]
    pub async fn update_upgrade_status(&mut self, target_status: EboUpgradeStatus) -> Result<(), EboServerError> {
        self.status.upgrade_status = Some(target_status);
        self.apply_status().await
    }

    pub fn get_password_reset_mode_status(&self) -> String {
        self.status.last_password_reset_mode.clone().unwrap_or_default()
    }

    pub async fn set_password_reset_mode_status(&mut self) -> Result<(), EboServerError> {
        let date = Utc::now().fixed_offset();
        self.status.last_password_reset_mode = Some(date.to_rfc3339());
        self.apply_status().await
    }

    pub fn get_reset_temporary_password_status(&self) -> String {
        self.status.last_reset_temporary_password.clone().unwrap_or_default()
    }

    pub async fn set_reset_temporary_password_status(&mut self) -> Result<(), EboServerError> {
        let date = Utc::now().fixed_offset();
        self.status.last_reset_temporary_password = Some(date.to_rfc3339());
        self.apply_status().await
    }

    pub async fn get_running_ebo_version(&self) -> &str {
        if let Some(img) = self.status.pod_image.as_ref() {
            img.rsplit(':').next().unwrap_or("")
        } else {
            ""
        }
    }

    async fn apply_status(&self) -> Result<(), EboServerError> {
        let patch = serde_json::json!({
            "apiVersion": EboServer::api_version(&()),
            "kind": EboServer::kind(&()),
            "metadata": {
                "name": self.name,
                "namespace": self.ns,
            },
            "status": Some(self.status.clone())
        });
        let ss_apply = PatchParams::apply(OPERATOR_NAME).force();
        let api = Api::<EboServer>::namespaced(self.client.clone(), &self.ns);
        api.patch_status(&self.name, &ss_apply, &Patch::Apply(patch))
            .await
            .map_err(EboServerError::KubeError)?;
        Ok(())
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn add_annotations(&self, annotations: &[(String, String)]) -> Result<(), kubeError> {
        let mut patch = serde_json::json!({
            "metadata": {
                "annotations": {}
            }
        });

        for (key, value) in annotations {
            patch["metadata"]["annotations"][key] = serde_json::Value::String(value.clone());
        }

        let ss_apply = PatchParams::apply(OPERATOR_NAME);
        let api = Api::<EboServer>::namespaced(self.client.clone(), &self.ns);
        api.patch(&self.name, &ss_apply, &Patch::Merge(patch)).await?;
        Ok(())
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn remove_annotations(&self, annotations: &[String]) -> Result<(), EboServerError> {
        let mut patch = serde_json::json!({
            "metadata": {
                "annotations": {}
            }
        });

        for key in annotations {
            patch["metadata"]["annotations"][key] = serde_json::Value::Null;
        }

        let ss_apply = PatchParams::apply(OPERATOR_NAME);
        let api = Api::<EboServer>::namespaced(self.client.clone(), &self.ns);
        api.patch(&self.name, &ss_apply, &Patch::Merge(patch))
            .await
            .map_err(EboServerError::KubeError)?;
        Ok(())
    }

    pub async fn get_annotation_value(&self, key: &str) -> Result<Option<String>, EboServerError> {
        let api = Api::<EboServer>::namespaced(self.client.clone(), &self.ns);
        let ebo_server = api.get(&self.name).await.map_err(EboServerError::KubeError)?;
        Ok(ebo_server
            .metadata
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.get(key).cloned()))
    }

    #[instrument(skip(self, mgmt_client), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn check_admin_password_ebo(&mut self, mgmt_client: &MGMTClient) -> Result<(), EboServerError> {
        let result = mgmt_client.check_admin_password_status(&self.name, &self.ns).await?;

        match result {
            EboAdminPasswordStatus::Active => {
                self.update_admin_password_status(EboAdminPasswordStatus::Active)
                    .await?;
                self.publish_event(OperatorEvent::TemporaryPasswordActive).await?;
            }
            EboAdminPasswordStatus::Expired => {
                self.publish_event(OperatorEvent::TemporaryPasswordExpired).await?;
                self.update_admin_password_status(EboAdminPasswordStatus::Expired)
                    .await?;
            }
            EboAdminPasswordStatus::Temporary => {
                tracing::info!("EBO still in temporary password status");
            }
        }
        Ok(())
    }

    #[instrument(skip(self, mgmt_client), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn create_backup(
        &mut self, mgmt_client: &MGMTClient, backups: i8, backup_name: String,
    ) -> Result<(), EboServerError> {
        let result = mgmt_client
            .create_backup(&self.name, &self.ns, backups, &backup_name)
            .await?;

        if result {
            self.publish_event(OperatorEvent::BackupStarted).await?;
        } else {
            self.publish_event(OperatorEvent::BackupFailed).await?;
        }
        Ok(())
    }

    #[instrument(skip(self, mgmt_client), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn backup_status(&mut self, mgmt_client: &MGMTClient) -> Result<bool, EboServerError> {
        let result = mgmt_client.backup_status(&self.name, &self.ns).await?;
        Ok(result)
    }

    #[instrument(skip(self, mgmt_client), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn backup_performed(
        &mut self, mgmt_client: &MGMTClient, backup_name: String,
    ) -> Result<bool, EboServerError> {
        let backups = mgmt_client
            .list_backups(&self.name, &self.ns)
            .await
            .map_err(EboServerError::from)?;

        // Check if the requested backup is present (exact match).
        let found = backups.iter().any(|b| b.starts_with(&backup_name));

        let event = if found {
            OperatorEvent::BackupSucceeded
        } else {
            OperatorEvent::BackupFailed
        };

        self.publish_event(event).await?;
        Ok(found)
    }

    #[instrument(skip(self, prometheus_client), fields(ebo = %self.name, ns = %self.ns))]
    pub async fn ok_to_perform_backup(
        &mut self, prometheus_client: &PrometheusClient, backups: i8,
    ) -> Result<bool, EboServerError> {
        let result = prometheus_client
            .ok_to_perform_backup(&self.name, &self.ns, backups)
            .await?;
        if !result {
            self.publish_event(OperatorEvent::OutOfSpaceBackup).await?;
        }
        Ok(result)
    }

    pub fn get_crash_loop_reported(&self) -> bool {
        self.status.crash_loop_reported.unwrap_or(false)
    }

    #[instrument(skip(self), fields(ebo = %self.name, ns = %self.ns, reported = %reported))]
    pub async fn set_crash_loop_reported(&mut self, reported: bool) -> Result<(), EboServerError> {
        self.status.crash_loop_reported = Some(reported);
        self.apply_status().await
    }
}

pub fn is_pod_running(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .is_some_and(|status| status.phase.as_ref().is_some_and(|phase| phase == "Running"))
}

pub fn is_pod_ready(p: &Pod) -> bool {
    p.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|conds| conds.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
        .unwrap_or(false)
}

pub fn is_crash_looping(p: &Pod) -> bool {
    p.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|cs| {
            cs.iter().any(|c| {
                c.state
                    .as_ref()
                    .and_then(|st| st.waiting.as_ref())
                    .and_then(|w| w.reason.as_deref())
                    == Some("CrashLoopBackOff")
            })
        })
        .unwrap_or(false)
}

pub fn total_restart_count(p: &Pod) -> i32 {
    p.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|cs| cs.iter().map(|c| c.restart_count).sum())
        .unwrap_or(0)
}

pub fn is_pod_terminating(pod: &Pod) -> bool {
    pod.metadata.deletion_timestamp.is_some()
}

pub fn is_pvc_being_deleted(pvc: &PersistentVolumeClaim) -> bool {
    pvc.metadata.deletion_timestamp.is_some()
}

pub fn is_pvc_pending(pvc: &PersistentVolumeClaim) -> bool {
    pvc.status
        .as_ref()
        .is_some_and(|status| status.phase.as_ref().is_some_and(|phase| phase == "Pending"))
}

pub fn has_pod_stopped(pod: &Pod) -> bool {
    pod.status.as_ref().is_some_and(|status| {
        status
            .phase
            .as_ref()
            .is_some_and(|phase| phase == "Failed" || phase == "Succeeded")
    })
}

pub fn has_job_succeeded(job: &Job) -> bool {
    job.status
        .as_ref()
        .is_some_and(|status| status.succeeded.is_some_and(|s| s > 0))
}
