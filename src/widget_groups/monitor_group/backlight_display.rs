use crate::{
    data::source::backlight::{Backlight, Kind},
    widget_groups::monitor_group::output_details::ConnectedOutput,
    widgets::{
        hardware_section::HardwareRows,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Row, Section, TooltipBody},
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
    let display_rows = [
        ("Display", backlight.serial.clone()),
        ("Type", kind.to_owned()),
        ("Brightness", brightness),
    ]
    .map(Row::from);
    let output_rows = output.map(ConnectedOutput::rows).unwrap_or_default();
    TooltipBody::sections(&[
        Section::untitled(&[display_rows.as_slice(), &output_rows].concat()),
        output.hardware_section(),
    ])
}
