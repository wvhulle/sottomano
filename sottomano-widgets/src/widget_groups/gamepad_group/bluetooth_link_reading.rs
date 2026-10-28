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
        let address = match self.identity {
            ControllerIdentity::Addressed {
                hardware_address, ..
            } => Some(KeyValue::new("Address", hardware_address.clone())),
            ControllerIdentity::Unaddressed => None,
        };
        TooltipBody::sections([Section::dictionary(
            [KeyValue::new("Carries", "input")]
                .into_iter()
                .chain(address),
        )])
    }
}
