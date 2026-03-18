use std::time::Duration;

use crate::crd::v1alpha6::{EboAdminPasswordStatus, EboServerPhase, EboUpgradeStatus};
use crate::events::definitions::OperatorEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition<P> {
    Stay,
    To(P),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision<P> {
    pub transition: Transition<P>,
    pub requeue: Duration,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Publish(OperatorEvent),

    UpdatePodStatus,

    SetBackupName(String),
    UpdateUpgradeJobStatus,
    UpdateUpgradeStatus(EboUpgradeStatus),
    EnsureUpgradeJob { name: String },
    TryStartBackup { name: String },
    CheckBackupSucceeded { name: String },

    UpdateRestore,
    UpdateRestoreJobStatus,
    EnsureRestoreJob { name: String, restore_name: String },
    SetUpgradeDisabled(bool),

    UpdatePasswordResetMode,
    UpdatePasswordResetJobStatus,
    EnsurePasswordResetJob { name: String },
    UpdateResetTemporaryPassword,
    UpdatePasswordStatus(EboAdminPasswordStatus),

    EnsureInitPasswordSecret,
    EnsurePod,
    DeletePod,
    DeleteInitPasswordSecret,
    DeleteDataPvc,

    CheckAdminPassword,

    UpdateCrashLoopReported(bool),

    ForcePhase(EboServerPhase),
}

#[derive(Debug, Clone, Copy)]
pub struct Flags {
    pub initial_password_enabled: bool,
}
