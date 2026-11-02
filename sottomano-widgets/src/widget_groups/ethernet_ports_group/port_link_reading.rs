use sottomano_sources::network_connectivity::{
    ethernet_connection::{Link, Port, PortState},
    interface_bus_attachment::InterfaceAttachment,
};

use crate::{
    clock::Clock,
    widget_groups::interface_address_sections::link_stats_section,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortMarking {
    UniqueAttachment,
    SharedAttachment { ordinal: u8 },
}

pub struct PortLinkReading<'a> {
    pub port_state: &'a PortState,
    pub marking: PortMarking,
}

pub struct PortConnectionSwitch<'a> {
    pub port_state: &'a PortState,
}

fn connection_description(port_state: &PortState) -> &'static str {
    if port_state.switching {
        "switching"
    } else if port_state.connected {
        "connected"
    } else {
        "disconnected"
    }
}

fn connection_state(port_state: &PortState) -> WidgetState {
    if port_state.switching {
        return WidgetState::Pending;
    }
    WidgetState::of_toggle(port_state.connected)
}

fn attachment_description(attachment: InterfaceAttachment) -> &'static str {
    match attachment {
        InterfaceAttachment::BuiltIn => "built-in",
        InterfaceAttachment::Usb => "USB adapter",
    }
}

impl GroupItemReading for PortLinkReading<'_> {
    fn name(&self) -> &'static str {
        "Link"
    }

    fn icon(&self) -> IconGlyph {
        match self.port_state.attachment {
            InterfaceAttachment::BuiltIn => Phosphor::Network(Weight::Regular).into(),
            InterfaceAttachment::Usb => Phosphor::Usb(Weight::Regular).into(),
        }
    }

    fn identity_ordinal(&self) -> Option<u8> {
        match self.marking {
            PortMarking::UniqueAttachment => None,
            PortMarking::SharedAttachment { ordinal } => Some(ordinal),
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        connection_state(self.port_state)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Port", self.port_state.name.as_str()),
            LabeledValue::new(
                "Attachment",
                attachment_description(self.port_state.attachment),
            ),
            LabeledValue::new("Connection", connection_description(self.port_state)),
        ])
    }
}

impl PortConnectionSwitch<'_> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.port_state.connected {
            "disconnect this Ethernet port"
        } else {
            "connect this Ethernet port"
        }
    }
}

impl GroupItemReading for PortConnectionSwitch<'_> {
    fn name(&self) -> &'static str {
        "Connection"
    }

    fn icon(&self) -> IconGlyph {
        if self.port_state.connected {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        connection_state(self.port_state)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Port", self.port_state.name.as_str()),
            LabeledValue::new("Connection", connection_description(self.port_state)),
        ])
    }
}

pub fn link_detail_sections(link: &Link) -> Vec<Section> {
    let profile_section = link
        .connection
        .as_deref()
        .map(|profile| Section::dictionary([LabeledValue::new("Network Profile", profile)]));
    profile_section
        .into_iter()
        .chain([link_stats_section(
            "Link",
            "Link Speed",
            link.speed,
            link.mtu,
            link.fault_counters,
        )])
        .collect()
}

pub fn port_identity_sections(port: &Port) -> Vec<Section> {
    let mac_section = port
        .link
        .as_ref()
        .map(|link| Section::dictionary([LabeledValue::new("MAC Address", link.mac.as_str())]));
    mac_section
        .into_iter()
        .chain(
            port.adapter
                .iter()
                .map(|adapter| Section::from(adapter).titled("Hardware")),
        )
        .collect()
}
