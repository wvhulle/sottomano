use sottomano_desktop_sources::network_connectivity::{
    ethernet_connection::{Link, Port, PortState},
    interface_bus_attachment::InterfaceAttachment,
    network_manager_proxies::LinkPhase,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::IdentityOrdinal,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::interface_address_sections::link_stats_section,
    widgets::value_cells::LabeledValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortMarking {
    UniqueAttachment,
    SharedAttachment { ordinal: u8 },
}

pub struct PortLinkIndicator<'a> {
    pub port_state: &'a PortState,
    pub marking: PortMarking,
}

pub struct PortConnectionSwitch<'a> {
    pub port_state: &'a PortState,
}

fn attachment_description(attachment: InterfaceAttachment) -> &'static str {
    match attachment {
        InterfaceAttachment::BuiltIn => "built-in",
        InterfaceAttachment::Usb => "USB adapter",
    }
}

impl<'a> PortLinkIndicator<'a> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'a> {
        let icon = match self.port_state.attachment {
            InterfaceAttachment::BuiltIn => Phosphor::Network(Weight::Regular).into(),
            InterfaceAttachment::Usb => Phosphor::Usb(Weight::Regular).into(),
        };
        let ordinal = match self.marking {
            PortMarking::UniqueAttachment => IdentityOrdinal::Unnumbered,
            PortMarking::SharedAttachment { ordinal } => IdentityOrdinal::Numbered(ordinal),
        };
        GroupIndicator::glyph_only(
            "Link",
            icon,
            IndicatorState::from(self.port_state.link_state),
        )
        .with_ordinal(ordinal)
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Port",
            dictionary_table([
                LabeledValue::new("Port", self.port_state.name.as_str()),
                LabeledValue::new(
                    "Attachment",
                    attachment_description(self.port_state.attachment),
                ),
                LabeledValue::new("Connection", self.port_state.link_state.label()),
            ]),
        )]
    }
}

impl PortConnectionSwitch<'_> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.port_state.link_state == LinkPhase::Up {
            "disconnect this Ethernet port"
        } else {
            "connect this Ethernet port"
        }
    }
}

impl<'a> PortConnectionSwitch<'a> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'a> {
        let icon = if self.port_state.link_state == LinkPhase::Up {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        };
        GroupIndicator::glyph_only(
            "Connection",
            icon,
            IndicatorState::from(self.port_state.link_state),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Port",
            dictionary_table([
                LabeledValue::new("Port", self.port_state.name.as_str()),
                LabeledValue::new("Connection", self.port_state.link_state.label()),
            ]),
        )]
    }
}

pub fn link_detail_sections(link: &Link) -> Vec<Section> {
    let profile_section = link.connection.as_deref().map(|profile| {
        Section::new(
            "Profile",
            dictionary_table([LabeledValue::new("Network Profile", profile)]),
        )
    });
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
    let mac_section = port.link.as_ref().map(|link| {
        Section::new(
            "Address",
            dictionary_table([LabeledValue::new("MAC Address", link.mac.as_str())]),
        )
    });
    mac_section
        .into_iter()
        .chain(port.adapter.iter().map(Section::from))
        .collect()
}
