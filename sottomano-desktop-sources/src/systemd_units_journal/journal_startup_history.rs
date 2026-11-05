use std::{io, time::Duration};

use serde::Deserialize;
use systemd::{id128::Id128, journal::OpenOptions};

use crate::{
    system_protocol_clients::journal_follow::decode_record,
    systemd_units_journal::{boot_timeline::OsStartupPhases, journal_record_fields::MessageId},
};

const SERVICE_MANAGER_PID: &str = "1";

#[derive(Debug, Deserialize)]
struct StartupFinishedRecord {
    #[serde(rename = "_BOOT_ID")]
    boot_id: String,
    #[serde(rename = "KERNEL_USEC")]
    kernel_usec: u64,
    #[serde(rename = "INITRD_USEC")]
    initrd_usec: Option<u64>,
    #[serde(rename = "USERSPACE_USEC")]
    userspace_usec: u64,
}

impl StartupFinishedRecord {
    fn os_startup(&self) -> OsStartupPhases {
        OsStartupPhases {
            kernel: Duration::from_micros(self.kernel_usec),
            initrd: self.initrd_usec.map(Duration::from_micros),
            userspace: Duration::from_micros(self.userspace_usec),
        }
    }
}

pub fn read_previous_os_startup() -> io::Result<Option<OsStartupPhases>> {
    let current_boot_id = Id128::from_boot()?.to_string();
    let mut journal = OpenOptions::default().system(true).open()?;
    MessageId::STARTUP_FINISHED.add_match(&mut journal)?;
    journal.match_add("_PID", SERVICE_MANAGER_PID)?;
    journal.seek_tail()?;
    while let Some(entry) = journal.previous_entry()? {
        let decoded = decode_record::<StartupFinishedRecord>(entry)
            .inspect_err(|e| tracing::debug!("systemd startup history: malformed record: {e}"))
            .ok()
            .filter(|record| record.boot_id != current_boot_id);
        if let Some(record) = decoded {
            return Ok(Some(record.os_startup()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_protocol_clients::journal_follow::journal_record;

    #[test]
    fn startup_record_splits_phases_and_tolerates_no_initrd() {
        let decode = |fields| {
            decode_record::<StartupFinishedRecord>(journal_record(fields))
                .expect("a startup record")
                .os_startup()
        };
        assert_eq!(
            decode(vec![
                ("_BOOT_ID", "b213fd5bcb4c447a9594128ce394fbba"),
                ("KERNEL_USEC", "581900"),
                ("INITRD_USEC", "5678549"),
                ("USERSPACE_USEC", "11973846"),
            ]),
            OsStartupPhases {
                kernel: Duration::from_micros(581_900),
                initrd: Some(Duration::from_micros(5_678_549)),
                userspace: Duration::from_micros(11_973_846),
            }
        );
        assert_eq!(
            decode(vec![
                ("_BOOT_ID", "e316057fe48e4fe6a55c9e43b3d76b7b"),
                ("KERNEL_USEC", "500000"),
                ("USERSPACE_USEC", "4000000"),
            ]),
            OsStartupPhases {
                kernel: Duration::from_millis(500),
                initrd: None,
                userspace: Duration::from_secs(4),
            }
        );
    }
}
