use sottomano_sources::gamepad_controller_input::evdev_gamepad_devices::ControllerIdentity;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct BluetoothLinkReading<'a> {
    pub identity: &'a ControllerIdentity,
}

impl BarWidget for BluetoothLinkReading<'_> {
    fn name(&self) -> &'static str {
        "Bluetooth connection"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Bluetooth(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let paired_identity = match self.identity {
            ControllerIdentity::Addressed {
                hardware_address,
                alias,
            } => vec![
                KeyValue::new("Paired as", alias.clone()),
                KeyValue::new("Address", hardware_address.clone()),
            ],
            ControllerIdentity::Unaddressed => Vec::new(),
        };
        TooltipBody::sections([Section::dictionary(
            paired_identity
                .into_iter()
                .chain([KeyValue::new("Carries", "input")]),
        )])
    }
}
