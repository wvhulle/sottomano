use uom::si::f64::Ratio;

use crate::{
    data::source::net_interfaces::bluetooth::{Bluetooth, Device},
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody, row, section},
        widget_state::{Level, WidgetState},
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
            self.best_signal().map_or(Level::Ok, signal_level).into()
        } else {
            WidgetState::Off
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.best_signal().map(percent_fill)
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
