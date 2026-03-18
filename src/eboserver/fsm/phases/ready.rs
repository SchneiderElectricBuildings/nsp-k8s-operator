use crate::crd::v1alpha6::{EboAdminPasswordStatus, EboServerPhase};
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, READY_REQUEUE, SHORT_REQUEUE};

pub fn step(o: &Observed) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::UpdatePodStatus];

    if !o.pod_exists() {
        effects.push(Effect::Publish(OperatorEvent::PodDeleted));
        return to(EboServerPhase::Starting, SHORT_REQUEUE, effects);
    }

    push_background_effects(o, &mut effects);

    if !o.pod_running {
        return stay(SHORT_REQUEUE, effects);
    }

    if o.crash_loop && !o.crash_loop_reported {
        effects.push(Effect::Publish(OperatorEvent::CrashLoopDetected));
        effects.push(Effect::UpdateCrashLoopReported(true));
        // You typically stay in Ready, but requeue short to react quickly
        return stay(SHORT_REQUEUE, effects);
    }

    if !o.crash_loop && o.crash_loop_reported {
        effects.push(Effect::Publish(OperatorEvent::CrashLoopRecovered));
        effects.push(Effect::UpdateCrashLoopReported(false));
        // continue normal flow
    }

    // move to password reset mode
    if o.pod_ready && o.last_password_reset_mode < o.password_reset_mode {
        effects.push(Effect::Publish(OperatorEvent::PasswordResetInitiated));
        return to(EboServerPhase::StoppingForPasswordReset, SHORT_REQUEUE, effects);
    }

    // move to phase reset temporary password
    if o.pod_ready && o.last_reset_temporary_password < o.reset_temporary_password {
        effects.push(Effect::DeleteInitPasswordSecret);
        return to(EboServerPhase::ResetTemporaryPassword, SHORT_REQUEUE, effects);
    }

    if o.pod_ready && o.last_restore < o.restore && !o.restore_name.is_empty() {
        effects.push(Effect::Publish(OperatorEvent::RestoreInitiated));
        return to(EboServerPhase::StoppingForPVCDelete, SHORT_REQUEUE, effects);
    }

    if o.drift_detected {
        if o.image_changed {
            if o.upgrade_disabled {
                return to(EboServerPhase::Ready, SHORT_REQUEUE, effects);
            } else {
                effects.push(Effect::Publish(OperatorEvent::ImageUpgrade));
                return to(EboServerPhase::PerformingBackup, SHORT_REQUEUE, effects);
            }
        }

        effects.push(Effect::Publish(OperatorEvent::SpecDriftDetected));
        return to(EboServerPhase::StoppingForSpecUpdate, SHORT_REQUEUE, effects);
    }

    if o.pvc_needs_resize {
        effects.push(Effect::Publish(OperatorEvent::VolumeResizeRequest));
        return to(EboServerPhase::StoppingForSpecUpdate, SHORT_REQUEUE, effects);
    }

    stay(READY_REQUEUE, effects)
}

fn push_background_effects(o: &Observed, effects: &mut Vec<Effect>) {
    if o.mgmt_enabled && o.pod_ready && o.admin_pwd_status == EboAdminPasswordStatus::Temporary {
        effects.push(Effect::CheckAdminPassword);
    }
}
