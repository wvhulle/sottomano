use std::path::Path;

use crate::system_protocol_clients::sysfs_attribute_reads::parse_field;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InterfaceFaultCounters {
    pub receive_errors: u64,
    pub transmit_errors: u64,
    pub receive_dropped: u64,
    pub transmit_dropped: u64,
}

impl InterfaceFaultCounters {
    #[must_use]
    pub fn of_interface(interface: &str) -> Self {
        let statistics_directory = Path::new("/sys/class/net")
            .join(interface)
            .join("statistics");
        let counter_named = |counter_file: &str| {
            parse_field(&statistics_directory.join(counter_file)).unwrap_or_default()
        };
        Self {
            receive_errors: counter_named("rx_errors"),
            transmit_errors: counter_named("tx_errors"),
            receive_dropped: counter_named("rx_dropped"),
            transmit_dropped: counter_named("tx_dropped"),
        }
    }
}
