use sottomano_desktop_sources::gamepad_controller_input::evdev_gamepad_devices::{
    GamepadTransport, InputRoute, PhysicalGamepadLink,
};

use crate::{
    hover_cards::hover_card_body::{Section, SectionWidget},
    widgets::value_cells::LabeledValue,
};

pub fn connection_rows(link: &PhysicalGamepadLink) -> Vec<LabeledValue> {
    let mut rows = Vec::new();
    if link.transport == GamepadTransport::SteamDongle {
        rows.push(LabeledValue::new("Link", "Steam Puck dongle"));
    }
    if link.input_route == InputRoute::SteamInput {
        rows.push(LabeledValue::new("Input", "Steam Input virtual pad"));
    }
    rows
}

pub fn steam_input_note(link: &PhysicalGamepadLink) -> Option<Section> {
    (link.input_route == InputRoute::SteamInput).then(|| {
        Section::new(
            "Held by Steam",
            SectionWidget::paragraph(
                "Steam has the controller open, so the kernel driver steps back. Buttons arrive \
                 through the virtual pad Steam creates, and the battery level is read from the \
                 reports the dongle sends to Steam, every 30 seconds."
                    .to_owned(),
            ),
        )
    })
}
