use crate::{
    data::source::backlight::{Backlight, Kind},
    widgets::{
        phosphor::{self, Phosphor},
        tooltip_body::{TooltipBody, row},
    },
};

pub fn face(backlight: &Backlight) -> Phosphor {
    match backlight.kind {
        Kind::Internal => phosphor::Laptop::Regular.into(),
        Kind::External => phosphor::Desktop::Regular.into(),
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
