use sottomano_desktop_sources::network_connectivity::{
    ethernet_connection::{ConnectionProfile, Link, PortLink, PortState},
    network_manager_proxies::LinkPhase,
};

use super::{EthernetPortsGroup, EthernetPortsInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::network_menu::interface_address_sections::link_stats_section,
    widgets::value_cells::LabeledValue,
};
pub struct PortConnectionSwitch<'view> {
    pub port_state: &'view PortState,
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

impl<'view> PortConnectionSwitch<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
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
    let mut sections = Vec::new();
    if let ConnectionProfile::Named(profile) = &link.connection {
        sections.push(Section::new(
            "Profile",
            dictionary_table([LabeledValue::new("Network Profile", profile.as_str())]),
        ));
    }
    sections.push(link_stats_section(
        "Link",
        "Link Speed",
        link.speed,
        link.mtu,
        link.fault_counters,
    ));
    sections
}

impl EthernetPortsGroup {
    pub(super) fn port_connection_row<'view>(
        &'view self,
        port_state: &'view PortState,
    ) -> GroupIndicator<'view> {
        let connection_switch = PortConnectionSwitch { port_state };
        let connection_press_does = connection_switch.press_does();
        connection_switch
            .indicator()
            .on_switch_press(
                port_state.link_state,
                connection_press_does,
                EthernetPortsInteraction::TogglePort(port_state.name.clone()),
            )
            .with_hover_card(Box::new(move |_| {
                self.port_connection_hover_card(port_state)
            }))
    }

    fn port_connection_hover_card(&self, port_state: &PortState) -> Vec<Section> {
        let link_details = self.port_detail_sections(&port_state.name, |port| {
            let PortLink::Up(link) = &port.link else {
                return Vec::new();
            };
            link_detail_sections(link)
        });
        let mut sections = self
            .ethernet
            .hover_card_with(PortConnectionSwitch { port_state }.hover_card());
        sections.extend(link_details);
        sections
    }
}
