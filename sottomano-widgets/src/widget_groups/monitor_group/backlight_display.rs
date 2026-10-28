use sottomano_sources::screen_brightness_color::clightd_backlight::{Backlight, BacklightKind};

use crate::{
    widget_groups::monitor_group::output_details::ConnectedOutput,
    widgets::{
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub fn face(backlight: &Backlight) -> Phosphor {
    match backlight.kind {
        BacklightKind::Internal => Phosphor::Laptop(Weight::Regular),
        BacklightKind::External => Phosphor::Desktop(Weight::Regular),
    }
}

pub fn tooltip(
    backlight: &Backlight,
    brightness: ValueCell,
    output: Option<&ConnectedOutput>,
) -> TooltipBody {
    let kind = match backlight.kind {
        BacklightKind::Internal => "Internal",
        BacklightKind::External => "External (DDC)",
    };
    let display_entries = [
        LabeledValue::new("Display", backlight.serial.as_str()),
        LabeledValue::new("Type", kind),
        LabeledValue::new("Brightness", brightness),
    ];
    let output_entries = output.into_iter().flat_map(ConnectedOutput::entries);
    TooltipBody::sections([
        Section::dictionary(display_entries.into_iter().chain(output_entries)),
        Section::hardware_identity(output.iter().flat_map(|output| output.hardware_entries())),
    ])
}
