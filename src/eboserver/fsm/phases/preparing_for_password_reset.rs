use crate::crd::v1alpha6::EboServerPhase;
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::UpdatePasswordResetJobStatus];

    if !o.password_reset_job_exists() {
        effects.push(Effect::EnsurePasswordResetJob {
            name: format!("{}-password-reset", o.name),
        });
        effects.push(Effect::Publish(OperatorEvent::PasswordResetJobCreated));
        return stay(SHORT_REQUEUE, effects);
    }

    if o.password_reset_job_succeeded {
        effects.push(Effect::Publish(OperatorEvent::PasswordResetJobSucceeded));
        effects.push(Effect::UpdatePasswordResetMode);
        return to(EboServerPhase::Starting, SHORT_REQUEUE, effects);
    }

    stay(SHORT_REQUEUE, effects)
}
