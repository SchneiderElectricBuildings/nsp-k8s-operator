use crate::crd::v1alpha6::EboServerPhase;
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed, target: EboServerPhase) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::UpdatePodStatus];

    if !o.pod_exists() {
        effects.push(Effect::Publish(OperatorEvent::PodDeleted));
        return to(target, SHORT_REQUEUE, effects);
    }

    if !o.pod_terminating {
        effects.push(Effect::DeletePod);
        effects.push(Effect::Publish(OperatorEvent::PodStopping));
        return stay(SHORT_REQUEUE, effects);
    }

    if o.pod_stopped {
        effects.push(Effect::Publish(OperatorEvent::PodStopped));
        return to(target, SHORT_REQUEUE, effects);
    }

    stay(SHORT_REQUEUE, effects)
}
