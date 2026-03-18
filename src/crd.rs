use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_inline_default::serde_inline_default;
use std::fmt;

use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::CustomResource;

pub mod v1alpha6 {
    use super::*;

    #[serde_inline_default]
    #[derive(CustomResource, Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Default)]
    #[kube(
        group = "digbld.em.se.com",
        version = "v1alpha6",
        kind = "EboServer",
        shortname = "ebo",
        status = "EboServerStatus",
        namespaced
    )]
    #[serde(rename_all = "camelCase")]
    pub struct EboServerSpec {
        #[serde(rename = "type")]
        pub type_: EboServerType,
        pub version: String,
        #[serde_inline_default(Some(false))]
        pub top_server: Option<bool>,
        #[serde_inline_default(Some(false))]
        pub inject_license: Option<bool>,
        #[serde(default)]
        pub db_storage: EboServerStorage,
        #[serde(default)]
        pub bckp_storage: EboServerStorage,
        #[serde_inline_default(EboServerServiceType::NodePort)]
        pub service_type: EboServerServiceType,
        #[serde(default = "default_resources")]
        pub resources: EboServerResources,
        #[serde_inline_default(Some("0 12 * * 1".to_string()))]
        pub backup_schedule: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub password_reset_mode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub reset_temporary_password: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub restore: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub restore_name: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Default)]
    pub enum EboServerType {
        EdgeServer,
        EnterpriseServer,
        EnterpriseCentral,
        #[default]
        EnterpriseServerCloud,
        EnterpriseCentralCloud,
    }

    impl fmt::Display for EboServerType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let ebo_type = match self {
                EboServerType::EdgeServer => "CS",
                EboServerType::EnterpriseServer => "ESC",
                EboServerType::EnterpriseCentral => "ECC",
                EboServerType::EnterpriseServerCloud => "ESCC",
                EboServerType::EnterpriseCentralCloud => "ECCC",
            };
            write!(f, "{ebo_type}")
        }
    }

    #[serde_inline_default]
    #[derive(Serialize, Deserialize, Default, Debug, Clone, JsonSchema, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct EboServerStorage {
        pub capacity: Option<Quantity>,
        pub storage_class_name: Option<String>,
        #[serde_inline_default(EboServerRetentionPolicy::Retain)]
        pub retention_policy: EboServerRetentionPolicy,
    }

    #[derive(Serialize, Deserialize, Default, Debug, PartialEq, Clone, JsonSchema)]
    pub enum EboServerRetentionPolicy {
        #[default]
        Retain,
        Delete,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, Default, PartialEq)]
    pub enum EboServerServiceType {
        #[serde(rename = "ClusterIP")]
        #[default]
        ClusterIp,
        NodePort,
    }

    #[serde_inline_default]
    #[derive(Serialize, Deserialize, Default, Debug, Clone, JsonSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct EboServerStatus {
        pub phase: Option<EboServerPhase>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub pod_phase: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub pod_image: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub upgrade_job_status: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub restore_job_status: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub password_reset_job_status: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub last_password_reset_mode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub last_reset_temporary_password: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub waf_status: Option<EboWafStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde_inline_default(Some(false))]
        pub registered: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub license_status: Option<EboLicenseStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub admin_password_status: Option<EboAdminPasswordStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub backup_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub upgrade_status: Option<EboUpgradeStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub last_restore: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde_inline_default(Some(false))]
        pub upgrade_disabled: Option<bool>,
        #[serde_inline_default(Some(true))]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub is_top_server: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde_inline_default(Some(false))]
        pub dns_records_created: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde_inline_default(Some(false))]
        pub crash_loop_reported: Option<bool>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Eq)]
    pub enum EboServerPhase {
        Starting,
        Ready,
        PerformingBackup,
        CheckingBackup,
        StoppingForUpgrade,
        PreparingForUpgrade,
        StoppingForSpecUpdate,
        PreparingForSpecUpdate,
        StoppingForPasswordReset,
        PreparingForPasswordReset,
        StoppingForPVCDelete,
        DeletingPVC,
        StartingForRestore,
        StoppingForRestore,
        PreparingForRestore,
        ResetTemporaryPassword,
    }

    #[derive(Default, Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
    pub enum EboWafStatus {
        Ready,
        Creating,
        #[default]
        NotReady,
    }

    impl Default for &EboWafStatus {
        fn default() -> Self {
            &EboWafStatus::NotReady
        }
    }

    #[derive(Default, Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
    pub enum EboLicenseStatus {
        #[default]
        NotActivated,
        Activated,
        Returned,
        Incorrect,
    }

    #[derive(Default, Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Eq, Copy)]
    pub enum EboAdminPasswordStatus {
        #[default]
        Temporary,
        Active,
        Expired,
    }

    #[derive(Default, Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Eq, Copy)]
    pub enum EboUpgradeStatus {
        #[default]
        Ready,
        BackupOngoing,
        BackupPerformed,
        BackupSucceeded,
        BackupFailed,
        UpgradeJobCreated,
        UpgradeJobSucceeded,
    }

    #[derive(Serialize, Deserialize, Default, Debug, Clone, JsonSchema, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct EboServerResources {
        #[serde(default = "default_limits")]
        pub limits: EboServerLimits,
        #[serde(default = "default_requests")]
        pub requests: EboServerRequests,
    }

    #[derive(Serialize, Deserialize, Default, Debug, Clone, JsonSchema, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct EboServerLimits {
        #[serde(default = "default_cpu_limit")]
        pub cpu: Quantity,
        #[serde(default = "default_memory_limit")]
        pub memory: Quantity,
    }

    #[derive(Serialize, Deserialize, Default, Debug, Clone, JsonSchema, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct EboServerRequests {
        #[serde(default = "default_cpu_request")]
        pub cpu: Quantity,
        #[serde(default = "default_memory_request")]
        pub memory: Quantity,
    }

    impl fmt::Display for EboServerPhase {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let phase_str = match self {
                EboServerPhase::Starting => "Starting",
                EboServerPhase::Ready => "Ready",
                EboServerPhase::StoppingForUpgrade => "StoppingForUpgrade",
                EboServerPhase::PerformingBackup => "PerformingBackup",
                EboServerPhase::CheckingBackup => "CheckingBackup",
                EboServerPhase::PreparingForUpgrade => "PreparingForUpgrade",
                EboServerPhase::StoppingForSpecUpdate => "StoppingForSpecUpdate",
                EboServerPhase::PreparingForSpecUpdate => "PreparingForSpecUpdate",
                EboServerPhase::StoppingForPasswordReset => "StoppingForPasswordReset",
                EboServerPhase::PreparingForPasswordReset => "PreparingForPasswordReset",
                EboServerPhase::StoppingForPVCDelete => "StoppingForPVCDelete",
                EboServerPhase::DeletingPVC => "DeletingPVC",
                EboServerPhase::StartingForRestore => "StartingForRestore",
                EboServerPhase::StoppingForRestore => "StoppingForRestore",
                EboServerPhase::PreparingForRestore => "PreparingForRestore",
                EboServerPhase::ResetTemporaryPassword => "ResetTemporaryPassword",
            };
            write!(f, "{phase_str}")
        }
    }

    impl fmt::Display for EboWafStatus {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let waf_status = match self {
                EboWafStatus::Ready => "Ready",
                EboWafStatus::Creating => "Creating",
                EboWafStatus::NotReady => "NotReady",
            };
            write!(f, "{waf_status}")
        }
    }

    impl fmt::Display for EboServerServiceType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                EboServerServiceType::ClusterIp => write!(f, "ClusterIP"),
                EboServerServiceType::NodePort => write!(f, "NodePort"),
            }
        }
    }

    // ---- Defaults used by the CRD ----

    pub fn default_cpu_limit() -> Quantity {
        Quantity("350m".into())
    }
    pub fn default_memory_limit() -> Quantity {
        Quantity("384Mi".into())
    }
    pub fn default_cpu_request() -> Quantity {
        Quantity("250m".into())
    }
    pub fn default_memory_request() -> Quantity {
        Quantity("256Mi".into())
    }

    pub fn default_limits() -> EboServerLimits {
        EboServerLimits {
            cpu: default_cpu_limit(),
            memory: default_memory_limit(),
        }
    }
    pub fn default_requests() -> EboServerRequests {
        EboServerRequests {
            cpu: default_cpu_request(),
            memory: default_memory_request(),
        }
    }
    pub fn default_resources() -> EboServerResources {
        EboServerResources {
            limits: default_limits(),
            requests: default_requests(),
        }
    }
}
