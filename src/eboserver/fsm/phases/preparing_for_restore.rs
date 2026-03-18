use crate::crd::v1alpha6::EboServerPhase;
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::UpdateRestoreJobStatus];

    if !o.restore_job_exists() {
        effects.push(Effect::EnsureRestoreJob {
            name: format!("{}-restore", o.name),
            restore_name: o.restore_name.clone(),
        });
        effects.push(Effect::Publish(OperatorEvent::RestoreJobCreated));
        return stay(SHORT_REQUEUE, effects);
    }

    if o.restore_job_succeeded {
        effects.push(Effect::Publish(OperatorEvent::RestoreJobSucceeded));
        effects.push(Effect::UpdateRestore);
        return to(EboServerPhase::Starting, SHORT_REQUEUE, effects);
    }

    stay(SHORT_REQUEUE, effects)
}
