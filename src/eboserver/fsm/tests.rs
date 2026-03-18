#![cfg(test)]
use crate::crd::v1alpha6::{EboAdminPasswordStatus, EboServerPhase, EboUpgradeStatus};
use crate::events::definitions::OperatorEvent;
use chrono::Local;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod, Secret};

use super::observe::{BackupOngoing, Observed};
use super::step;
use super::types::{Decision, Effect, Flags, Transition};

mod harness {
    use super::*;

    pub fn assert_transition(d: &Decision<EboServerPhase>, expected: Transition<EboServerPhase>, case: &str) {
        assert_eq!(d.transition, expected, "case={case} effects={:?}", d.effects);
    }

    pub fn assert_has(d: &Decision<EboServerPhase>, e: Effect, case: &str) {
        assert!(d.effects.contains(&e), "case={case} missing={e:?} effects={:?}", d.effects);
    }

    pub fn assert_not_has(d: &Decision<EboServerPhase>, e: Effect, case: &str) {
        assert!(!d.effects.contains(&e), "case={case} unexpected={e:?} effects={:?}", d.effects);
    }

    pub fn assert_requeue_secs(d: &Decision<EboServerPhase>, secs: u64, case: &str) {
        assert_eq!(
            d.requeue.as_secs(),
            secs,
            "case={case} transition={:?} effects={:?}",
            d.transition,
            d.effects
        );
    }
}

fn some_pod() -> Pod {
    Default::default()
}
fn some_secret() -> Secret {
    Default::default()
}
fn some_job() -> Job {
    Default::default()
}
fn some_pvc() -> PersistentVolumeClaim {
    Default::default()
}

#[derive(Clone)]
struct Obs(Observed);

impl Obs {
    fn base() -> Self {
        Self(Observed {
            name: "ebo1".into(),
            deployed_by: "syncer".into(),
            top_server: true,
            password_reset_mode: "".to_owned(),
            reset_temporary_password: "".to_owned(),

            pod: None,
            init_pass_secret: None,
            data_pvc: None,
            upgrade_job: None,
            upgrade_job_succeeded: false,

            restore_name: "".to_owned(),
            restore: "".to_owned(),
            last_restore: "".to_owned(),
            restore_job: None,
            restore_job_succeeded: false,

            password_reset_job: None,
            password_reset_job_succeeded: false,
            last_password_reset_mode: "".to_owned(),
            last_reset_temporary_password: "".to_owned(),

            pod_running: false,
            pod_ready: false,
            crash_loop: false,
            restart_count: 0,
            crash_loop_reported: false,

            pod_terminating: false,
            pod_stopped: false,

            drift_detected: false,
            image_changed: false,
            pvc_needs_resize: false,

            admin_pwd_status: EboAdminPasswordStatus::Temporary,
            backup_name: "".to_owned(),
            upgrade_status: EboUpgradeStatus::Ready,

            mgmt_enabled: true,

            backup_ongoing: BackupOngoing::Known(false),
            upgrade_disabled: false,
        })
    }

    fn build(self) -> Observed {
        self.0
    }

    fn name(mut self, v: &str) -> Self {
        self.0.name = v.into();
        self
    }

    fn pod_exists(mut self) -> Self {
        self.0.pod = Some(some_pod());
        self
    }
    fn pod_running(mut self) -> Self {
        self.0.pod_running = true;
        self
    }
    fn pod_ready(mut self, v: bool) -> Self {
        self.0.pod_ready = v;
        self
    }

    fn pod_terminating(mut self) -> Self {
        self.0.pod_terminating = true;
        self
    }
    fn pod_stopped(mut self) -> Self {
        self.0.pod_stopped = true;
        self
    }

    fn secret_exists(mut self) -> Self {
        self.0.init_pass_secret = Some(some_secret());
        self
    }

    fn drift(mut self) -> Self {
        self.0.drift_detected = true;
        self
    }
    fn image_changed(mut self) -> Self {
        self.0.image_changed = true;
        self
    }
    fn pvc_resize(mut self) -> Self {
        self.0.data_pvc = Some(some_pvc());
        self.0.pvc_needs_resize = true;
        self
    }

    fn admin_pwd_status(mut self, v: EboAdminPasswordStatus) -> Self {
        self.0.admin_pwd_status = v;
        self
    }

    fn clients_enabled(mut self, mgmt: bool) -> Self {
        self.0.mgmt_enabled = mgmt;
        self
    }

    fn backup_ongoing(mut self, v: BackupOngoing) -> Self {
        self.0.backup_ongoing = v;
        self
    }
    fn upgrade_status(mut self, v: EboUpgradeStatus) -> Self {
        self.0.upgrade_status = v;
        self
    }

    fn backup_name(mut self, v: &str) -> Self {
        self.0.backup_name = v.into();
        self
    }

    fn reset_temporary_password(mut self, v: &str) -> Self {
        self.0.reset_temporary_password = v.into();
        self
    }

    fn last_reset_temporary_password(mut self, v: &str) -> Self {
        self.0.last_reset_temporary_password = v.into();
        self
    }

    fn upgrade_job_exists(mut self) -> Self {
        self.0.upgrade_job = Some(some_job());
        self
    }
    fn upgrade_job_succeeded(mut self) -> Self {
        self.0.upgrade_job_succeeded = true;
        self
    }

    fn password_reset_job_exists(mut self) -> Self {
        self.0.password_reset_job = Some(some_job());
        self
    }

    fn password_reset_job_succeeded(mut self) -> Self {
        self.0.password_reset_job_succeeded = true;
        self
    }

    fn restore_name(mut self, v: &str) -> Self {
        self.0.restore_name = v.into();
        self
    }

    fn restore(mut self, v: &str) -> Self {
        self.0.restore = v.into();
        self
    }

    fn last_restore(mut self, v: &str) -> Self {
        self.0.last_restore = v.into();
        self
    }

    fn restore_job_exists(mut self) -> Self {
        self.0.restore_job = Some(some_job());
        self
    }

    fn restore_job_succeeded(mut self) -> Self {
        self.0.restore_job_succeeded = true;
        self
    }
}

// Starting

#[test]
fn starting_table() {
    use harness::*;

    struct Case {
        name: &'static str,
        obs: Obs,
        flags: Flags,
        tr: Transition<EboServerPhase>,
        must: Vec<Effect>,
        must_not: Vec<Effect>,
        requeue_secs: u64,
    }

    let cases = vec![
        Case {
            name: "missing pod -> ensure pod",
            obs: Obs::base(),
            flags: Flags {
                initial_password_enabled: false,
            },
            tr: Transition::Stay,
            must: vec![
                Effect::UpdatePodStatus,
                Effect::EnsurePod,
                Effect::Publish(OperatorEvent::PodCreated),
            ],
            must_not: vec![Effect::Publish(OperatorEvent::PodReady)],
            requeue_secs: 15,
        },
        Case {
            name: "pod ready -> to Ready",
            obs: Obs::base().pod_exists().pod_running().pod_ready(true),
            flags: Flags {
                initial_password_enabled: false,
            },
            tr: Transition::To(EboServerPhase::Ready),
            must: vec![Effect::UpdatePodStatus, Effect::Publish(OperatorEvent::PodReady)],
            must_not: vec![Effect::EnsurePod],
            requeue_secs: 15,
        },
        Case {
            name: "password enabled + secret missing -> ensure secret",
            obs: Obs::base(),
            flags: Flags {
                initial_password_enabled: true,
            },
            tr: Transition::Stay,
            must: vec![
                Effect::EnsureInitPasswordSecret,
                Effect::Publish(OperatorEvent::PasswordSecretCreated),
            ],
            must_not: vec![],
            requeue_secs: 15,
        },
        Case {
            name: "password enabled + secret exists -> no ensure secret",
            obs: Obs::base().secret_exists(),
            flags: Flags {
                initial_password_enabled: true,
            },
            tr: Transition::Stay,
            must: vec![Effect::UpdatePodStatus],
            must_not: vec![Effect::EnsureInitPasswordSecret],
            requeue_secs: 15,
        },
    ];

    for c in cases {
        let d = step(EboServerPhase::Starting, &c.obs.build(), c.flags);
        assert_transition(&d, c.tr, c.name);
        assert_requeue_secs(&d, c.requeue_secs, c.name);
        for e in c.must {
            assert_has(&d, e, c.name);
        }
        for e in c.must_not {
            assert_not_has(&d, e, c.name);
        }
    }
}

// Ready

#[test]
fn ready_pod_missing_always_goes_starting_invariant() {
    use harness::*;

    let o = Obs::base()
        .drift()
        .image_changed()
        .pvc_resize()
        .clients_enabled(true)
        .build();
    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Starting), "invariant");
    assert_has(&d, Effect::Publish(OperatorEvent::PodDeleted), "invariant");
    assert_requeue_secs(&d, 15, "invariant");
}

#[test]
fn ready_pod_not_running_stays_short() {
    use harness::*;

    let o = Obs::base().pod_exists().build();
    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "pod_not_running");
    assert_requeue_secs(&d, 15, "pod_not_running");
}

#[test]
fn ready_drift_image_changed_goes_to_backup() {
    use harness::*;

    let o = Obs::base().pod_exists().pod_running().drift().image_changed().build();
    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::PerformingBackup), "drift_image");
    assert_has(&d, Effect::Publish(OperatorEvent::ImageUpgrade), "drift_image");
    assert_requeue_secs(&d, 15, "drift_image");
}

#[test]
fn ready_drift_without_image_goes_to_spec_update_stop() {
    use harness::*;

    let o = Obs::base().pod_exists().pod_running().drift().build();
    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::StoppingForSpecUpdate), "drift_spec");
    assert_has(&d, Effect::Publish(OperatorEvent::SpecDriftDetected), "drift_spec");
    assert_requeue_secs(&d, 15, "drift_spec");
}

#[test]
fn ready_pvc_resize_goes_to_spec_update_stop() {
    use harness::*;

    let o = Obs::base().pod_exists().pod_running().pvc_resize().build();
    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::StoppingForSpecUpdate), "pvc_resize");
    assert_has(&d, Effect::Publish(OperatorEvent::VolumeResizeRequest), "pvc_resize");
}

#[test]
fn ready_priority_drift_image_beats_pvc_resize() {
    use harness::*;

    let o = Obs::base()
        .pod_exists()
        .pod_running()
        .drift()
        .image_changed()
        .pvc_resize()
        .build();
    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::PerformingBackup), "priority");
    assert_has(&d, Effect::Publish(OperatorEvent::ImageUpgrade), "priority");
    assert_not_has(&d, Effect::Publish(OperatorEvent::VolumeResizeRequest), "priority");
}

#[test]
fn ready_background_admin_password_check_conditions() {
    use harness::*;

    let o = Obs::base()
        .pod_exists()
        .pod_running()
        .pod_ready(true)
        .clients_enabled(true)
        .admin_pwd_status(EboAdminPasswordStatus::Temporary)
        .build();

    let d = step(
        EboServerPhase::Ready,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );
    assert_has(&d, Effect::CheckAdminPassword, "admin_pwd");
}

// PerformingBackup

#[test]
fn performing_backup_mgmt_disabled_skips_to_stopping_for_upgrade() {
    use harness::*;

    let o = Obs::base().backup_ongoing(BackupOngoing::Disabled).build();

    let d = step(
        EboServerPhase::PerformingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::StoppingForUpgrade), "mgmt_disabled");
}
#[test]
fn performing_backup_ongoing_waits() {
    use harness::*;

    let o = Obs::base().backup_ongoing(BackupOngoing::Known(true)).build();

    let d = step(
        EboServerPhase::PerformingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "ongoing");
}

#[test]
fn performing_backup_ready_starts_backup_and_sets_status() {
    use harness::*;

    let o = Obs::base()
        .backup_ongoing(BackupOngoing::Known(false))
        .upgrade_status(EboUpgradeStatus::Ready)
        .build();

    let d = step(
        EboServerPhase::PerformingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    let backup_name = Local::now().format("%d%m%y-%H%M").to_string();
    assert_transition(&d, Transition::Stay, "start_backup");
    assert_has(
        &d,
        Effect::TryStartBackup {
            name: backup_name.to_string(),
        },
        "start_backup",
    );

    // Variant A: status becomes PerformingBackup only if create_backup succeeded (in act),
    // therefore decide must NOT emit this update.
    assert_not_has(&d, Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupOngoing), "start_backup");
}

#[test]
fn performing_backup_unknown_waits() {
    use harness::*;

    let o = Obs::base().backup_ongoing(BackupOngoing::Unknown).build();

    let d = step(
        EboServerPhase::PerformingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "unknown_waits");
}

#[test]
fn performing_backup_not_ready_finishes_and_moves_on() {
    use harness::*;

    let o = Obs::base()
        .backup_ongoing(BackupOngoing::Known(false))
        .upgrade_status(EboUpgradeStatus::BackupOngoing)
        .build();

    let d = step(
        EboServerPhase::PerformingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::CheckingBackup), "finish_backup");
    assert_has(&d, Effect::Publish(OperatorEvent::BackupPerformed), "finish_backup");
}

// Stopping

#[test]
fn stopping_for_spec_update_pod_missing_goes_to_preparing_spec_update() {
    use harness::*;

    let o = Obs::base().build();
    let d = step(
        EboServerPhase::StoppingForSpecUpdate,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::PreparingForSpecUpdate), "pod_missing");
    assert_has(&d, Effect::Publish(OperatorEvent::PodDeleted), "pod_missing");
}

#[test]
fn stopping_for_upgrade_deletes_pod_when_not_terminating() {
    use harness::*;

    let o = Obs::base().pod_exists().pod_running().build();
    let d = step(
        EboServerPhase::StoppingForUpgrade,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "delete_pod");
    assert_has(&d, Effect::DeletePod, "delete_pod");
    assert_has(&d, Effect::Publish(OperatorEvent::PodStopping), "delete_pod");
}

#[test]
fn stopping_for_upgrade_moves_to_preparing_when_pod_stopped() {
    use harness::*;

    let o = Obs::base().pod_exists().pod_terminating().pod_stopped().build();
    let d = step(
        EboServerPhase::StoppingForUpgrade,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::PreparingForUpgrade), "pod_stopped");
    assert_has(&d, Effect::Publish(OperatorEvent::PodStopped), "pod_stopped");
}

// PreparingForUpgrade / PreparingForSpecUpdate

#[test]
fn preparing_for_upgrade_creates_job_when_missing() {
    use harness::*;

    let o = Obs::base().name("ebo-x").build();
    let d = step(
        EboServerPhase::PreparingForUpgrade,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "job_missing");
    assert_has(
        &d,
        Effect::EnsureUpgradeJob {
            name: "ebo-x-prepare-upgrade".into(),
        },
        "job_missing",
    );
    assert_has(&d, Effect::Publish(OperatorEvent::UpgradeJobCreated), "job_missing");
}

#[test]
fn preparing_for_upgrade_transitions_to_starting_on_success() {
    use harness::*;

    let o = Obs::base().upgrade_job_exists().upgrade_job_succeeded().build();
    let d = step(
        EboServerPhase::PreparingForUpgrade,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Starting), "job_succeeded");
    assert_has(&d, Effect::Publish(OperatorEvent::UpgradeJobSucceeded), "job_succeeded");
}

#[test]
fn preparing_for_spec_update_immediately_returns_to_starting() {
    use harness::*;

    let o = Obs::base().build();
    let d = step(
        EboServerPhase::PreparingForSpecUpdate,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Starting), "spec_update");
    assert_requeue_secs(&d, 60, "spec_update");
}

// CheckingBackup

#[test]
fn checking_backup_backup_performed_checks_backup() {
    use harness::*;

    let o = Obs::base()
        .backup_name("ebo1_010101-0101")
        .upgrade_status(EboUpgradeStatus::BackupPerformed)
        .build();
    let d = step(
        EboServerPhase::CheckingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "backup_performed");
    assert_has(
        &d,
        Effect::CheckBackupSucceeded {
            name: "ebo1_010101-0101".into(),
        },
        "backup_performed",
    );
    assert_requeue_secs(&d, 15, "backup_performed");
}

#[test]
fn checking_backup_backup_succeeded_moves_to_stopping_for_upgrade() {
    use harness::*;

    let o = Obs::base().upgrade_status(EboUpgradeStatus::BackupSucceeded).build();
    let d = step(
        EboServerPhase::CheckingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::StoppingForUpgrade), "backup_succeeded");
    assert_requeue_secs(&d, 15, "backup_succeeded");
}

#[test]
fn checking_backup_backup_ongoing_marks_performed() {
    use harness::*;

    let o = Obs::base().upgrade_status(EboUpgradeStatus::BackupOngoing).build();
    let d = step(
        EboServerPhase::CheckingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "backup_ongoing");
    assert_has(&d, Effect::Publish(OperatorEvent::BackupPerformed), "backup_ongoing");
    assert_has(
        &d,
        Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupPerformed),
        "backup_ongoing",
    );
    assert_requeue_secs(&d, 15, "backup_ongoing");
}

#[test]
fn checking_backup_backup_failed_disables_upgrade_and_returns_ready() {
    use harness::*;

    let o = Obs::base().upgrade_status(EboUpgradeStatus::BackupFailed).build();
    let d = step(
        EboServerPhase::CheckingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Ready), "backup_failed");
    assert_has(&d, Effect::SetBackupName("".into()), "backup_failed");
    assert_has(&d, Effect::SetUpgradeDisabled(true), "backup_failed");
    assert_has(&d, Effect::Publish(OperatorEvent::BackupFailed), "backup_failed");
    assert_has(&d, Effect::Publish(OperatorEvent::UpgradeFailed), "backup_failed");
    assert_requeue_secs(&d, 15, "backup_failed");
}

#[test]
fn checking_backup_unhandled_status_stays_without_effects() {
    use harness::*;

    let o = Obs::base().upgrade_status(EboUpgradeStatus::Ready).build();
    let d = step(
        EboServerPhase::CheckingBackup,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "unhandled");
    assert!(d.effects.is_empty(), "case=unhandled effects={:?}", d.effects);
    assert_requeue_secs(&d, 15, "unhandled");
}

// DeletingPVC

#[test]
fn deleting_pvc_for_temp_password_reset_goes_to_starting_and_updates_flags() {
    use harness::*;

    let o = Obs::base()
        .last_reset_temporary_password("0")
        .reset_temporary_password("1")
        .build();
    let d = step(
        EboServerPhase::DeletingPVC,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Starting), "temp_reset");
    assert_has(&d, Effect::DeleteDataPvc, "temp_reset");
    assert_has(&d, Effect::UpdateResetTemporaryPassword, "temp_reset");
    assert_has(
        &d,
        Effect::UpdatePasswordStatus(EboAdminPasswordStatus::Temporary),
        "temp_reset",
    );
    assert_has(&d, Effect::Publish(OperatorEvent::TemporaryPasswordReset), "temp_reset");
}

#[test]
fn deleting_pvc_for_restore_goes_to_starting_for_restore_without_temp_reset_effects() {
    use harness::*;

    let o = Obs::base()
        .last_reset_temporary_password("1")
        .reset_temporary_password("1")
        .last_restore("0")
        .restore("1")
        .restore_name("backup-1")
        .build();
    let d = step(
        EboServerPhase::DeletingPVC,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::StartingForRestore), "restore");
    assert_has(&d, Effect::DeleteDataPvc, "restore");
    assert_not_has(&d, Effect::UpdateResetTemporaryPassword, "restore");
    assert_not_has(&d, Effect::UpdatePasswordStatus(EboAdminPasswordStatus::Temporary), "restore");
    assert_not_has(&d, Effect::Publish(OperatorEvent::TemporaryPasswordReset), "restore");
}

// PreparingForPasswordReset

#[test]
fn preparing_for_password_reset_creates_job_when_missing() {
    use harness::*;

    let o = Obs::base().name("ebo-reset").build();
    let d = step(
        EboServerPhase::PreparingForPasswordReset,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "job_missing");
    assert_has(&d, Effect::UpdatePasswordResetJobStatus, "job_missing");
    assert_has(
        &d,
        Effect::EnsurePasswordResetJob {
            name: "ebo-reset-password-reset".into(),
        },
        "job_missing",
    );
    assert_has(&d, Effect::Publish(OperatorEvent::PasswordResetJobCreated), "job_missing");
}

#[test]
fn preparing_for_password_reset_moves_to_starting_on_success() {
    use harness::*;

    let o = Obs::base()
        .password_reset_job_exists()
        .password_reset_job_succeeded()
        .build();
    let d = step(
        EboServerPhase::PreparingForPasswordReset,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Starting), "job_succeeded");
    assert_has(&d, Effect::Publish(OperatorEvent::PasswordResetJobSucceeded), "job_succeeded");
    assert_has(&d, Effect::UpdatePasswordResetMode, "job_succeeded");
}

#[test]
fn preparing_for_password_reset_waits_when_job_not_succeeded() {
    use harness::*;

    let o = Obs::base().password_reset_job_exists().build();
    let d = step(
        EboServerPhase::PreparingForPasswordReset,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "job_waiting");
    assert_has(&d, Effect::UpdatePasswordResetJobStatus, "job_waiting");
    assert_not_has(&d, Effect::Publish(OperatorEvent::PasswordResetJobSucceeded), "job_waiting");
    assert_not_has(
        &d,
        Effect::EnsurePasswordResetJob {
            name: "ebo1-password-reset".into(),
        },
        "job_waiting",
    );
}

// PreparingForRestore

#[test]
fn preparing_for_restore_creates_job_when_missing() {
    use harness::*;

    let o = Obs::base().name("ebo-restore").restore_name("bk-123").build();
    let d = step(
        EboServerPhase::PreparingForRestore,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "job_missing");
    assert_has(&d, Effect::UpdateRestoreJobStatus, "job_missing");
    assert_has(
        &d,
        Effect::EnsureRestoreJob {
            name: "ebo-restore-restore".into(),
            restore_name: "bk-123".into(),
        },
        "job_missing",
    );
    assert_has(&d, Effect::Publish(OperatorEvent::RestoreJobCreated), "job_missing");
}

#[test]
fn preparing_for_restore_moves_to_starting_on_success() {
    use harness::*;

    let o = Obs::base().restore_job_exists().restore_job_succeeded().build();
    let d = step(
        EboServerPhase::PreparingForRestore,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::To(EboServerPhase::Starting), "job_succeeded");
    assert_has(&d, Effect::Publish(OperatorEvent::RestoreJobSucceeded), "job_succeeded");
    assert_has(&d, Effect::UpdateRestore, "job_succeeded");
}

#[test]
fn preparing_for_restore_waits_when_job_not_succeeded() {
    use harness::*;

    let o = Obs::base().restore_job_exists().build();
    let d = step(
        EboServerPhase::PreparingForRestore,
        &o,
        Flags {
            initial_password_enabled: false,
        },
    );

    assert_transition(&d, Transition::Stay, "job_waiting");
    assert_has(&d, Effect::UpdateRestoreJobStatus, "job_waiting");
    assert_not_has(&d, Effect::Publish(OperatorEvent::RestoreJobSucceeded), "job_waiting");
    assert_not_has(
        &d,
        Effect::EnsureRestoreJob {
            name: "ebo1-restore".into(),
            restore_name: "".into(),
        },
        "job_waiting",
    );
}

fn extract_forced_transition(effects: &[Effect]) -> Option<Transition<EboServerPhase>> {
    let mut forced: Option<Transition<EboServerPhase>> = None;
    for e in effects {
        if let Effect::ForcePhase(p) = e {
            forced = Some(Transition::To(p.clone()));
        }
    }
    forced
}

#[test]
fn force_transition_none_when_no_effect_present() {
    let effects = vec![
        Effect::UpdatePodStatus,
        Effect::TryStartBackup {
            name: "test".to_string(),
        },
    ];
    let forced = extract_forced_transition(&effects);
    assert_eq!(forced, None);
}

#[test]
fn force_transition_present_when_effect_present() {
    let effects = vec![Effect::UpdatePodStatus, Effect::ForcePhase(EboServerPhase::Ready)];
    let forced = extract_forced_transition(&effects);
    assert_eq!(forced, Some(Transition::To(EboServerPhase::Ready)));
}

#[test]
fn force_transition_last_wins_when_multiple_present() {
    let effects = vec![
        Effect::ForcePhase(EboServerPhase::Ready),
        Effect::UpdatePodStatus,
        Effect::ForcePhase(EboServerPhase::StoppingForUpgrade),
    ];
    let forced = extract_forced_transition(&effects);
    assert_eq!(forced, Some(Transition::To(EboServerPhase::StoppingForUpgrade)));
}
