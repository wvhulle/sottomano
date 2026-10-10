use uom::si::f64::Ratio;

use crate::{
    data::{
        severity::Severity,
        source::net_interfaces::bluetooth::{Bluetooth, Device},
    },
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Bluetooth {
    fn icon(&self) -> Glyph {
        Phosphor::Bluetooth(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        if !self.powered {
            return None;
        }
        u32::try_from(self.connected.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        if self.powered {
            self.best_signal()
                .map_or(Severity::Normal, signal_level)
                .into()
        } else {
            WidgetState::Off
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.best_signal().map(percent_fill)
    }

    fn tooltip(&self) -> TooltipBody {
        let hardware = self.adapter.hardware_section();
        if !self.powered {
            return TooltipBody::sections(&[
                Section::untitled_dictionary([KeyValue::new("Bluetooth", "off")]),
                hardware,
            ]);
        }
        let overview = Section::dictionary(
            "Bluetooth",
            [
                KeyValue::new("Connected", self.connected.len().to_string()),
                KeyValue::new("Paired", self.paired.to_string()),
            ],
        );
        let devices = self.connected.iter().map(device_section);
        TooltipBody::sections(&[vec![overview], devices.collect(), vec![hardware]].concat())
    }
}

fn device_section(device: &Device) -> Section {
    Section::dictionary(
        device.name.clone(),
        [
            device
                .address
                .as_deref()
                .map(|address| KeyValue::new("Address", address)),
            device
                .signal
                .map(|signal| KeyValue::new("Signal", format!("{signal}%"))),
            device
                .battery
                .map(|battery| KeyValue::new("Battery", format!("{battery}%"))),
        ]
        .into_iter()
        .flatten(),
    )
}
