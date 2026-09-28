use crate::{
    data::source::backlight::{Backlight, Kind},
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

pub fn tooltip(backlight: &Backlight, brightness: String) -> TooltipBody {
    let kind = match backlight.kind {
        Kind::Internal => "Internal",
        Kind::External => "External (DDC)",
    };
    TooltipBody::rows(vec![
        row("Display", backlight.serial.clone()),
        row("Type", kind),
        row("Brightness", brightness),
    ])
}
