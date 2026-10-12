use crate::{
    data::source::backlight::{Backlight, Kind},
    widget_groups::monitor_group::output_details::ConnectedOutput,
    widgets::{
        hardware_section::HardwareEntries,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
    },
};

pub fn face(backlight: &Backlight) -> Phosphor {
    match backlight.kind {
        Kind::Internal => Phosphor::Laptop(Weight::Regular),
        Kind::External => Phosphor::Desktop(Weight::Regular),
    }
}

pub fn tooltip(
    backlight: &Backlight,
    brightness: String,
    output: Option<&ConnectedOutput>,
) -> TooltipBody {
    let kind = match backlight.kind {
        Kind::Internal => "Internal",
        Kind::External => "External (DDC)",
    };
    let display_entries = [
        KeyValue::new("Display", backlight.serial.as_str()),
        KeyValue::new("Type", kind),
        KeyValue::new("Brightness", brightness),
    ];
    let output_entries = output.into_iter().flat_map(ConnectedOutput::entries);
    TooltipBody::sections([
        Section::dictionary(display_entries.into_iter().chain(output_entries)),
        output.hardware_section(),
    ])
}
