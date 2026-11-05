use std::path::Path;

use super::CPU_FREQ;
use crate::system_protocol_clients::sysfs_attribute_reads::read_trimmed;

pub(super) fn turbo_enabled() -> Option<bool> {
    read_trimmed(Path::new(CPU_FREQ).join("intel_pstate/no_turbo")).map(|no_turbo| no_turbo != "1")
}
