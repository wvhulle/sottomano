use winnow::{
    Parser, Result,
    ascii::{dec_uint, hex_uint, line_ending, multispace0, till_line_ending},
    combinator::{alt, delimited, preceded, repeat, terminated},
    error::{ContextError, ParseError},
    token::take_till,
};

const VALVE_USB_VENDOR: u16 = 0x28de;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VirtualSlotControllerType {
    SteamController,
    OtherController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamVirtualSlot {
    pub slot: u8,
    pub physical_name: String,
    pub physical_vendor: u16,
    pub physical_product: u16,
    pub controller_type: VirtualSlotControllerType,
    pub steam_handle: u64,
}

impl SteamVirtualSlot {
    #[must_use]
    pub fn is_steam_controller(&self) -> bool {
        self.controller_type == VirtualSlotControllerType::SteamController
            && self.physical_vendor == VALVE_USB_VENDOR
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Steam virtual gamepad table malformed at byte {offset}: {reason}")]
pub struct VirtualSlotTableSyntaxError {
    pub offset: usize,
    pub reason: String,
}

impl From<ParseError<&str, ContextError>> for VirtualSlotTableSyntaxError {
    fn from(parse_failure: ParseError<&str, ContextError>) -> Self {
        VirtualSlotTableSyntaxError {
            offset: parse_failure.offset(),
            reason: parse_failure.inner().to_string(),
        }
    }
}

#[derive(Clone)]
enum SlotField<'input> {
    Name(&'input str),
    Vendor(u16),
    Product(u16),
    ControllerType(VirtualSlotControllerType),
    Handle(u64),
    Unknown,
}

fn hexadecimal_prefix(input: &mut &str) -> Result<()> {
    alt(("0x", "0X")).void().parse_next(input)
}

fn slot_field<'input>(input: &mut &'input str) -> Result<SlotField<'input>> {
    let key = terminated(take_till(1.., ['=', '\n', '[']), '=').parse_next(input)?;
    match key {
        "name" => till_line_ending.map(SlotField::Name).parse_next(input),
        "VID" => preceded(hexadecimal_prefix, hex_uint)
            .map(SlotField::Vendor)
            .parse_next(input),
        "PID" => preceded(hexadecimal_prefix, hex_uint)
            .map(SlotField::Product)
            .parse_next(input),
        "handle" => preceded(hexadecimal_prefix, hex_uint)
            .map(SlotField::Handle)
            .parse_next(input),
        "type" => till_line_ending
            .map(|controller_type: &str| {
                SlotField::ControllerType(if controller_type == "steam" {
                    VirtualSlotControllerType::SteamController
                } else {
                    VirtualSlotControllerType::OtherController
                })
            })
            .parse_next(input),
        _ => till_line_ending.value(SlotField::Unknown).parse_next(input),
    }
}

fn slot_header(input: &mut &str) -> Result<u8> {
    delimited("[slot ", dec_uint, ']').parse_next(input)
}

fn slot_block(input: &mut &str) -> Result<SteamVirtualSlot> {
    (
        terminated(slot_header, multispace0),
        repeat(0.., terminated(slot_field, (line_ending, multispace0))),
    )
        .map(|(slot, fields): (u8, Vec<SlotField<'_>>)| {
            let mut virtual_slot = SteamVirtualSlot {
                slot,
                physical_name: String::new(),
                physical_vendor: 0,
                physical_product: 0,
                controller_type: VirtualSlotControllerType::OtherController,
                steam_handle: 0,
            };
            for field in fields {
                match field {
                    SlotField::Name(name) => name.clone_into(&mut virtual_slot.physical_name),
                    SlotField::Vendor(vendor) => virtual_slot.physical_vendor = vendor,
                    SlotField::Product(product) => virtual_slot.physical_product = product,
                    SlotField::ControllerType(controller_type) => {
                        virtual_slot.controller_type = controller_type;
                    }
                    SlotField::Handle(handle) => virtual_slot.steam_handle = handle,
                    SlotField::Unknown => {}
                }
            }
            virtual_slot
        })
        .parse_next(input)
}

pub fn parse_virtual_slot_table(
    text: &str,
) -> Result<Vec<SteamVirtualSlot>, VirtualSlotTableSyntaxError> {
    preceded(multispace0, repeat(0.., slot_block))
        .parse(text)
        .map_err(VirtualSlotTableSyntaxError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_SLOTS: &str = "[slot 0]\nname=Steam \
                             Controller\nVID=0x28de\nPID=0x1302\nhandle=0x0000fa0000000001\\
                             ntype=steam\n[slot 1]\nname=DualSense Wireless \
                             Controller\nVID=0x054c\nPID=0x0ce6\nhandle=0x0000000000000002\\
                             ntype=ps5\n";

    #[test]
    fn each_slot_names_its_physical_controller() {
        let slots = parse_virtual_slot_table(TWO_SLOTS).expect("valid table");
        assert_eq!(
            slots[0],
            SteamVirtualSlot {
                slot: 0,
                physical_name: "Steam Controller".to_owned(),
                physical_vendor: 0x28de,
                physical_product: 0x1302,
                controller_type: VirtualSlotControllerType::SteamController,
                steam_handle: 0xfa00_0000_0001,
            }
        );
        assert_eq!(slots[1].slot, 1);
        assert_eq!(
            slots[1].controller_type,
            VirtualSlotControllerType::OtherController
        );
    }

    #[test]
    fn only_valve_slots_of_type_steam_are_steam_controllers() {
        let slots = parse_virtual_slot_table(TWO_SLOTS).expect("valid table");
        assert!(slots[0].is_steam_controller());
        assert!(!slots[1].is_steam_controller());
    }

    #[test]
    fn an_empty_table_has_no_slots() {
        assert!(
            parse_virtual_slot_table("")
                .expect("valid table")
                .is_empty()
        );
    }
}
