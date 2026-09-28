use crate::{
    data::source::net_interfaces::bluetooth::{Bluetooth, Device},
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        phosphor,
        tooltip_body::{Section, TooltipBody, row, section},
    },
};

impl BarWidget for Bluetooth {
    fn icon(&self) -> Glyph {
        phosphor::Bluetooth::Regular.into()
    }

    fn disabled(&self) -> bool {
        !self.powered
    }

    fn badge(&self) -> Option<u32> {
        if !self.powered {
            return None;
        }
        u32::try_from(self.connected.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        self.best_signal().map_or(Level::Ok, signal_level)
    }

    fn value(&self) -> Option<Value> {
        self.best_signal().map(|percent| Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.powered {
            return TooltipBody::rows(vec![row("Bluetooth", "off")]);
        }
        let mut sections = vec![section(
            "Bluetooth",
            vec![
                row("Connected", self.connected.len().to_string()),
                row("Paired", self.paired.to_string()),
            ],
        )];
        sections.extend(self.connected.iter().map(device_section));
        TooltipBody::Sections(sections)
    }
}

fn device_section(device: &Device) -> Section {
    let mut rows = Vec::new();
    rows.extend(
        device
            .address
            .as_ref()
            .map(|address| row("Address", address.clone())),
    );
    rows.extend(
        device
            .signal
            .map(|signal| row("Signal", format!("{signal}%"))),
    );
    rows.extend(
        device
            .battery
            .map(|battery| row("Battery", format!("{battery}%"))),
    );
    section(device.name.clone(), rows)
}
