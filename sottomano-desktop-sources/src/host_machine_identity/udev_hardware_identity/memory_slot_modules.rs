use bytesize::ByteSize;

use crate::host_machine_identity::udev_hardware_identity::{HardwareText, UdevProperties};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryModule {
    pub manufacturer: HardwareText,
    pub part_number: HardwareText,
    pub kind: HardwareText,
    pub transfer_rate: MemoryTransferRate,
    pub size: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryTransferRate {
    Reported { megatransfers_per_second: u32 },
    Unreported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySlotModule {
    pub locator: String,
    pub module: MemoryModule,
}

pub(super) fn memory_modules(properties: &UdevProperties) -> Vec<MemorySlotModule> {
    (0..)
        .map_while(|slot| MemorySlot::read(properties, slot))
        .filter_map(MemorySlot::populated)
        .collect()
}

enum MemorySlot {
    Populated(MemorySlotModule),
    Empty,
}

impl MemorySlot {
    fn read(properties: &UdevProperties, slot: usize) -> Option<Self> {
        let key = |field: &str| format!("MEMORY_DEVICE_{slot}_{field}");
        let locator = properties.properties_by_key.get(&key("LOCATOR"))?;
        let Some(size) = properties
            .value(&key("SIZE"))
            .and_then(|bytes| {
                bytes
                    .parse()
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "memory modules: invalid size {bytes:?} of slot {slot}: \
                             {parse_failure}"
                        );
                    })
                    .ok()
            })
            .map(ByteSize::b)
        else {
            return Some(MemorySlot::Empty);
        };
        Some(MemorySlot::Populated(MemorySlotModule {
            locator: locator.clone(),
            module: MemoryModule {
                manufacturer: properties.text(&key("MANUFACTURER")),
                part_number: properties.text(&key("PART_NUMBER")),
                kind: properties.text(&key("TYPE")),
                transfer_rate: slot_transfer_rate(properties, slot),
                size,
            },
        }))
    }

    fn populated(self) -> Option<MemorySlotModule> {
        match self {
            MemorySlot::Populated(slot_module) => Some(slot_module),
            MemorySlot::Empty => None,
        }
    }
}

fn slot_transfer_rate(properties: &UdevProperties, slot: usize) -> MemoryTransferRate {
    let Some(speed) = properties
        .value(&format!("MEMORY_DEVICE_{slot}_CONFIGURED_SPEED_MTS"))
        .or_else(|| properties.value(&format!("MEMORY_DEVICE_{slot}_SPEED_MTS")))
    else {
        return MemoryTransferRate::Unreported;
    };
    match speed.parse() {
        Ok(megatransfers_per_second) => MemoryTransferRate::Reported {
            megatransfers_per_second,
        },
        Err(parse_failure) => {
            tracing::warn!(
                "memory modules: invalid speed {speed:?} of slot {slot}: {parse_failure}"
            );
            MemoryTransferRate::Unreported
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLDERED_LPDDR5: &str = "\
E:MEMORY_DEVICE_0_SIZE=4294967296
E:MEMORY_DEVICE_0_LOCATOR=Controller0-ChannelA
E:MEMORY_DEVICE_0_TYPE=LPDDR5
E:MEMORY_DEVICE_0_CONFIGURED_SPEED_MTS=8533
E:MEMORY_DEVICE_0_MANUFACTURER=Samsung
E:MEMORY_DEVICE_0_PART_NUMBER=Not Specified
E:MEMORY_DEVICE_1_SIZE=4294967296
E:MEMORY_DEVICE_1_LOCATOR=Controller0-ChannelB
E:MEMORY_DEVICE_1_TYPE=LPDDR5
E:MEMORY_DEVICE_1_CONFIGURED_SPEED_MTS=8533
E:MEMORY_DEVICE_1_MANUFACTURER=Samsung
E:MEMORY_DEVICE_2_LOCATOR=Controller1-ChannelA
E:MEMORY_ARRAY_LOCATION=System Board Or Motherboard
";

    #[test]
    fn populated_slots_are_kept_per_locator_and_empty_slots_skipped() {
        let modules = memory_modules(&UdevProperties::parse(SOLDERED_LPDDR5));
        let samsung_lpddr5 = MemorySlotModule {
            locator: "Controller0-ChannelA".to_owned(),
            module: MemoryModule {
                manufacturer: HardwareText::Reported("Samsung".to_owned()),
                part_number: HardwareText::Unreported,
                kind: HardwareText::Reported("LPDDR5".to_owned()),
                transfer_rate: MemoryTransferRate::Reported {
                    megatransfers_per_second: 8533,
                },
                size: ByteSize::gib(4),
            },
        };
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0], samsung_lpddr5);
        assert_eq!(modules[1].locator, "Controller0-ChannelB");
    }
}
