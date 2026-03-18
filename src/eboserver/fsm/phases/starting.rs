use crate::crd::v1alpha6::EboServerPhase;
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect, Flags};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed, flags: Flags, target: EboServerPhase) -> Decision<EboServerPhase> {
    let mut effects = vec![Effect::UpdatePodStatus];

    if flags.initial_password_enabled && !o.secret_exists() {
        effects.push(Effect::EnsureInitPasswordSecret);
        effects.push(Effect::Publish(OperatorEvent::PasswordSecretCreated));
    }

    if !o.pod_exists() {
        effects.push(Effect::EnsurePod);
        effects.push(Effect::Publish(OperatorEvent::PodCreated));
        return stay(SHORT_REQUEUE, effects);
    }

    if o.pod_ready {
        effects.push(Effect::Publish(OperatorEvent::PodReady));
        return to(target, SHORT_REQUEUE, effects);
    }

    stay(SHORT_REQUEUE, effects)
}
