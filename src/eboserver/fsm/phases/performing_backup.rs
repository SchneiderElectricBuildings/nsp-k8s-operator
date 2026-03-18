use crate::crd::v1alpha6::{EboServerPhase, EboUpgradeStatus};
use crate::events::definitions::OperatorEvent;
use chrono::Local;

use crate::eboserver::fsm::observe::{BackupOngoing, Observed};
use crate::eboserver::fsm::types::{Decision, Effect};
use crate::eboserver::fsm::{stay, to, SHORT_REQUEUE};

pub fn step(o: &Observed) -> Decision<EboServerPhase> {
    match o.backup_ongoing {
        BackupOngoing::Disabled => {
            return to(
                EboServerPhase::StoppingForUpgrade,
                SHORT_REQUEUE,
                vec![Effect::Publish(OperatorEvent::BackupSucceeded)], //cannot publish that a backup is taken if its not?
            );
        }

        BackupOngoing::Unknown => {
            return stay(SHORT_REQUEUE, vec![]);
        }

        BackupOngoing::Known(true) => {
            return stay(SHORT_REQUEUE, vec![]);
        }

        BackupOngoing::Known(false) => {}
    }

    if o.upgrade_status == EboUpgradeStatus::Ready {
        // get name of backup, if not null
        let backup_name: &str = if o.backup_name.is_empty() {
            &Local::now().format("%d%m%y-%H%M").to_string()
        } else {
            &o.backup_name
        };

        return stay(
            SHORT_REQUEUE,
            vec![
                Effect::TryStartBackup {
                    name: backup_name.to_string(),
                },
                Effect::SetBackupName(backup_name.to_string()),
            ],
        );
    }

    to(
        EboServerPhase::CheckingBackup,
        SHORT_REQUEUE,
        vec![Effect::Publish(OperatorEvent::BackupPerformed)],
    )
}
