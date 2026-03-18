use crate::crd::v1alpha6::EboServerPhase;

use crate::eboserver::fsm::types::Decision;
use crate::eboserver::fsm::{to, READY_REQUEUE};

pub fn step() -> Decision<EboServerPhase> {
    to(EboServerPhase::Starting, READY_REQUEUE, vec![])
}
