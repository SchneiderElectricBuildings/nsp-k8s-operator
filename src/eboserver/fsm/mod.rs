use std::time::Duration;

use crate::crd::v1alpha6::EboServerPhase;

pub const SHORT_REQUEUE: Duration = Duration::from_secs(15);
pub const READY_REQUEUE: Duration = Duration::from_secs(60);

pub fn stay<P>(requeue: Duration, effects: Vec<types::Effect>) -> types::Decision<P> {
    types::Decision {
        transition: types::Transition::Stay,
        requeue,
        effects,
    }
}

pub fn to<P>(phase: P, requeue: Duration, effects: Vec<types::Effect>) -> types::Decision<P> {
    types::Decision {
        transition: types::Transition::To(phase),
        requeue,
        effects,
    }
}

pub mod act;
pub mod observe;
pub mod types;

pub mod phases;

pub fn step(phase: EboServerPhase, o: &observe::Observed, flags: types::Flags) -> types::Decision<EboServerPhase> {
    phases::step(phase, o, flags)
}

#[cfg(test)]
mod tests;
