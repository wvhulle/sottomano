use std::iter::once;

use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::bluez_bluetooth::Bluetooth;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Bluetooth {
    fn name(&self) -> &'static str {
        "Bluetooth"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Bluetooth(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.radio.as_ref()?.connected.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.power_switching {
            return WidgetState::Pending;
        }
        self.radio.as_ref().map_or(WidgetState::Off, |radio| {
            radio
                .best_signal()
                .map_or(Severity::Normal, signal_level)
                .into()
        })
    }

    fn gauge(&self) -> Option<Ratio> {
        self.radio.as_ref()?.best_signal().map(percent_fill)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let hardware = self.adapter.iter().map(Section::from);
        let Some(radio) = &self.radio else {
            return TooltipBody::sections(
                [Section::dictionary([KeyValue::new("Bluetooth", "off")])]
                    .into_iter()
                    .chain(hardware),
            );
        };
        let overview = Section::dictionary([
            KeyValue::new("Connected", radio.connected.len().to_string()),
            KeyValue::new("Paired", radio.paired.to_string()),
        ])
        .titled("Bluetooth");
        let devices = radio.connected.iter().map(|device| {
            Section::dictionary(
                [KeyValue::new("Address", device.address.as_str())]
                    .into_iter()
                    .chain(
                        device
                            .signal
                            .map(|signal| KeyValue::new("Signal", format!("{signal}%"))),
                    )
                    .chain(
                        device
                            .battery
                            .map(|battery| KeyValue::new("Battery", format!("{battery}%"))),
                    ),
            )
            .titled(device.name.clone())
        });
        TooltipBody::sections(once(overview).chain(devices).chain(hardware))
    }
}
