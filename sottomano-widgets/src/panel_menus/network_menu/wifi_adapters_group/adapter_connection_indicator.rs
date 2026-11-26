use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::{LinkPhase, RadioPower},
    wifi_connection::{AdapterAddressing, WifiAdapter, WifiAdapterDetails},
};

use super::{WifiAdaptersGroup, WifiAdaptersInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::network_menu::interface_address_sections::link_stats_section,
    widgets::value_cells::LabeledValue,
};

pub struct AdapterConnectionSwitch<'view> {
    pub adapter: &'view WifiAdapter,
}

impl<'view> AdapterConnectionSwitch<'view> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.adapter.link_state == LinkPhase::Up {
            "disconnect this Wi-Fi adapter"
        } else {
            "connect this Wi-Fi adapter"
        }
    }

    pub fn indicator(self) -> GroupIndicator<'view> {
        let glyph = if self.adapter.link_state == LinkPhase::Up {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        };
        GroupIndicator::glyph_only(
            "Connection",
            glyph,
            IndicatorState::from(self.adapter.link_state),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Adapter",
            dictionary_table([
                LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                LabeledValue::new("Connection", self.adapter.link_state.label()),
            ]),
        )]
    }
}

pub fn adapter_radio_stats_sections(details: &WifiAdapterDetails) -> Vec<Section> {
    let AdapterAddressing::Addressed(addressing) = &details.addressing else {
        return Vec::new();
    };
    vec![link_stats_section(
        "Radio",
        "Radio Speed",
        addressing.bitrate,
        addressing.mtu,
        addressing.fault_counters,
    )]
}

impl WifiAdaptersGroup {
    pub(super) fn adapter_connection_rows<'view>(
        &'view self,
        adapter: &'view WifiAdapter,
        radio_power: RadioPower,
    ) -> Vec<GroupIndicator<'view>> {
        if radio_power != RadioPower::On {
            return Vec::new();
        }
        let connection_switch = AdapterConnectionSwitch { adapter };
        let connection_press_does = connection_switch.press_does();
        vec![
            connection_switch
                .indicator()
                .on_switch_press(
                    adapter.link_state,
                    connection_press_does,
                    WifiAdaptersInteraction::ToggleAdapter(adapter.device.clone()),
                )
                .with_hover_card(Box::new(move |_| {
                    self.adapter_connection_hover_card(adapter)
                })),
        ]
    }

    fn adapter_connection_hover_card(&self, adapter: &WifiAdapter) -> Vec<Section> {
        let mut sections = self
            .wifi
            .hover_card_with(AdapterConnectionSwitch { adapter }.hover_card());
        sections
            .extend(self.adapter_detail_sections(&adapter.device, adapter_radio_stats_sections));
        sections
    }
}
