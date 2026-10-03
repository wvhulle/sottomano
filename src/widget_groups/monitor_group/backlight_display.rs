use crate::{
    data::source::backlight::{Backlight, Kind},
    widget_groups::monitor_group::output_details::ConnectedOutput,
    widgets::{
        phosphor::{Phosphor, Weight},
        tooltip_body::{TooltipBody, row},
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
    output: Option<ConnectedOutput>,
) -> TooltipBody {
    let kind = match backlight.kind {
        Kind::Internal => "Internal",
        Kind::External => "External (DDC)",
    };
    let mut rows = vec![
        row("Display", backlight.serial.clone()),
        row("Type", kind),
        row("Brightness", brightness),
    ];
    rows.extend(output.into_iter().flat_map(|output| output.rows()));
    TooltipBody::rows(rows)
}
