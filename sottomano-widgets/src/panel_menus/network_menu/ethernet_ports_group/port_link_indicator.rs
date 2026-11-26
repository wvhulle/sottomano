use sottomano_desktop_sources::network_connectivity::{
    ethernet_connection::{Port, PortLink, PortState},
    interface_bus_attachment::{AdapterIdentity, InterfaceAttachment},
};

use super::{EthernetPortsBusRequest, EthernetPortsGroup, EthernetPortsInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::IdentityOrdinal,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortMarking {
    UniqueAttachment,
    SharedAttachment { ordinal: u8 },
}

pub struct PortLinkIndicator<'view> {
    pub port_state: &'view PortState,
    pub marking: PortMarking,
}

fn attachment_description(attachment: InterfaceAttachment) -> &'static str {
    match attachment {
        InterfaceAttachment::BuiltIn => "built-in",
        InterfaceAttachment::Usb => "USB adapter",
    }
}

impl<'view> PortLinkIndicator<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
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

pub fn port_identity_sections(port: &Port) -> Vec<Section> {
    let mut sections = Vec::new();
    if let PortLink::Up(link) = &port.link {
        sections.push(Section::new(
            "Address",
            dictionary_table([LabeledValue::new("MAC Address", link.mac.as_str())]),
        ));
    }
    if let AdapterIdentity::Identified(adapter) = &port.adapter {
        sections.push(Section::from(adapter));
    }
    sections
}

impl EthernetPortsGroup {
    pub(super) fn port_link_row<'view>(
        &'view self,
        port_state: &'view PortState,
    ) -> GroupIndicator<'view> {
        PortLinkIndicator {
            port_state,
            marking: self.port_marking(port_state),
        }
        .indicator()
        .with_hover_card(Box::new(move |_| self.port_link_hover_card(port_state)))
        .fetching_on_hover(
            EthernetPortsInteraction::OnBuses(EthernetPortsBusRequest::StartDetailsFetch),
            EthernetPortsInteraction::StopHoverFetches,
        )
    }

    fn port_link_hover_card(&self, port_state: &PortState) -> Vec<Section> {
        let link_indicator = PortLinkIndicator {
            port_state,
            marking: self.port_marking(port_state),
        };
        let mut sections = self.ethernet.hover_card_with(link_indicator.hover_card());
        sections.extend(self.port_detail_sections(&port_state.name, port_identity_sections));
        sections
    }
}
