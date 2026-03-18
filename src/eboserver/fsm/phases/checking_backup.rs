use crate::crd::v1alpha6::{EboServerPhase, EboUpgradeStatus};
use crate::events::definitions::OperatorEvent;

use crate::eboserver::fsm::observe::Observed;
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed) -> Decision<EboServerPhase> {
    let backup_name = &o.backup_name;

    // needs to be stored somewhere at CRD
    match o.upgrade_status {
        EboUpgradeStatus::BackupPerformed => stay(
            SHORT_REQUEUE,
            vec![Effect::CheckBackupSucceeded {
                name: backup_name.to_string(),
            }],
        ),
        EboUpgradeStatus::BackupSucceeded => to(EboServerPhase::StoppingForUpgrade, SHORT_REQUEUE, vec![]),
        EboUpgradeStatus::BackupOngoing => stay(
            SHORT_REQUEUE,
            vec![
                Effect::Publish(OperatorEvent::BackupPerformed),
                Effect::UpdateUpgradeStatus(EboUpgradeStatus::BackupPerformed),
            ],
        ),
        EboUpgradeStatus::BackupFailed => to(
            EboServerPhase::Ready,
            SHORT_REQUEUE,
            vec![
                Effect::SetBackupName("".to_owned()),
                Effect::SetUpgradeDisabled(true),
                Effect::Publish(OperatorEvent::BackupFailed),
                Effect::Publish(OperatorEvent::UpgradeFailed),
            ],
        ),

        // Catch-all for anything not handled above (including future variants).
        other => {
            tracing::warn!(?other, "Unhandled upgrade status in this phase; requeueing");
            stay(SHORT_REQUEUE, vec![])
        }
    }
}
