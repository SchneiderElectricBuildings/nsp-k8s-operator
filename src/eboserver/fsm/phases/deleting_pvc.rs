use crate::crd::v1alpha6::{EboAdminPasswordStatus, EboServerPhase};
use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{to, SHORT_REQUEUE};
use crate::events::definitions::OperatorEvent;

pub fn step(o: &Observed, target: EboServerPhase) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::DeleteDataPvc];
    // Delete data pvc and send to target, often start of some kind

    if o.last_reset_temporary_password < o.reset_temporary_password {
        effects.push(Effect::UpdateResetTemporaryPassword);
        effects.push(Effect::UpdatePasswordStatus(EboAdminPasswordStatus::Temporary));
        effects.push(Effect::Publish(OperatorEvent::TemporaryPasswordReset));
    }

    to(target, SHORT_REQUEUE, effects)
}
