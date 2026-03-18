use crate::crd::v1alpha6::{EboServerPhase, EboUpgradeStatus};
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::UpdateUpgradeJobStatus];

    if !o.upgrade_job_exists() {
        effects.push(Effect::EnsureUpgradeJob {
            name: format!("{}-prepare-upgrade", o.name),
        });
        effects.push(Effect::Publish(OperatorEvent::UpgradeJobCreated));
        effects.push(Effect::UpdateUpgradeStatus(EboUpgradeStatus::Ready));
        return stay(SHORT_REQUEUE, effects);
    }

    if o.upgrade_job_succeeded {
        effects.push(Effect::Publish(OperatorEvent::UpgradeJobSucceeded));
        return to(EboServerPhase::Starting, SHORT_REQUEUE, effects);
    }

    stay(SHORT_REQUEUE, effects)
}
