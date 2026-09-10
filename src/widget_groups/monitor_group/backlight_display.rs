use crate::{
    data::source::backlight::{Backlight, Kind},
    widgets::{
        font_awesome::{FontAwesome, Solid},
        tooltip_body::{TooltipBody, row},
    },
};

pub fn face(backlight: &Backlight) -> FontAwesome {
    match backlight.kind {
        Kind::Internal => Solid::Laptop.into(),
        Kind::External => Solid::Desktop.into(),
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
