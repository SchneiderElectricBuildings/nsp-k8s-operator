use crate::crd::v1alpha6::EboServerPhase;

use super::observe::Observed;
use super::types::{Decision, Flags};

mod checking_backup;
mod deleting_pvc;
mod performing_backup;
mod preparing_for_password_reset;
mod preparing_for_restore;
mod preparing_for_spec_update;
mod preparing_for_upgrade;
mod ready;
mod starting;
mod stopping;

pub fn step(phase: EboServerPhase, o: &Observed, flags: Flags) -> Decision<EboServerPhase> {
    match phase {
        EboServerPhase::Starting => starting::step(o, flags, EboServerPhase::Ready),
        EboServerPhase::Ready => ready::step(o),
        EboServerPhase::PerformingBackup => performing_backup::step(o),
        EboServerPhase::CheckingBackup => checking_backup::step(o),
        EboServerPhase::StoppingForUpgrade => stopping::step(o, EboServerPhase::PreparingForUpgrade),
        EboServerPhase::StoppingForSpecUpdate => stopping::step(o, EboServerPhase::PreparingForSpecUpdate),
        EboServerPhase::StoppingForPasswordReset => stopping::step(o, EboServerPhase::PreparingForPasswordReset),
        EboServerPhase::PreparingForUpgrade => preparing_for_upgrade::step(o),
        EboServerPhase::PreparingForSpecUpdate => preparing_for_spec_update::step(),
        EboServerPhase::PreparingForPasswordReset => preparing_for_password_reset::step(o),
        EboServerPhase::StoppingForPVCDelete => stopping::step(o, EboServerPhase::DeletingPVC),
        EboServerPhase::StartingForRestore => starting::step(o, flags, EboServerPhase::StoppingForRestore),
        EboServerPhase::StoppingForRestore => stopping::step(o, EboServerPhase::PreparingForRestore),
        EboServerPhase::PreparingForRestore => preparing_for_restore::step(o),
        EboServerPhase::ResetTemporaryPassword => stopping::step(o, EboServerPhase::StoppingForPVCDelete),

        EboServerPhase::DeletingPVC => {
            if o.last_reset_temporary_password < o.reset_temporary_password {
                deleting_pvc::step(o, EboServerPhase::Starting)
            } else if o.last_restore < o.restore && !o.restore_name.is_empty() {
                deleting_pvc::step(o, EboServerPhase::StartingForRestore)
            } else {
                starting::step(o, flags, EboServerPhase::Ready)
            }
        }
    }
}
