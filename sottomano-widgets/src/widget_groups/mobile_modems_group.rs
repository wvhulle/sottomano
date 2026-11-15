mod modem_signal_reading;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::{
    toggle_mobile_modem, toggle_mobile_radio,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        modem_manager_mobile::{Mobile, MobileModems},
        network_manager_proxies::{LinkPhase, RadioPower},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use self::modem_signal_reading::{
    AllMobileRadiosSwitch, ModemConnectionSwitch, ModemDataReading, ModemSignalReading,
    modem_family_address,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        interface_address_sections::{AddressFamily, InterfaceAddressReading},
        widget_group_content::{KeyedExpansions, WidgetGroupContent, WidgetGroupExpansionMessage},
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{subscribed_snapshot::SubscribedSnapshot, value_cells::LabeledValue},
};

#[derive(Debug)]
pub enum MobileModemsMessage {
    Snapshot(SnapshotDelivery<MobileModems>),
}

#[derive(Debug, Clone)]
pub enum MobileModemsInteraction {
    ToggleModem(String),
    ToggleAllRadios,
    Expansion {
        modem_path: String,
        expansion: WidgetGroupExpansionMessage,
    },
}

#[derive(Default)]
pub struct MobileModemsGroup {
    mobile_modems: SubscribedSnapshot<MobileModems>,
    expansions: KeyedExpansions<String>,
}

impl MobileModemsGroup {
    pub fn collapse(&mut self) {
        self.expansions.collapse();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<MobileModemsMessage> {
        snapshot_subscription::<MobileModems>(buses.clone()).map(MobileModemsMessage::Snapshot)
    }

    pub fn update(
        &mut self,
        message: MobileModemsMessage,
        clock: Clock,
    ) -> Task<MobileModemsMessage> {
        match message {
            MobileModemsMessage::Snapshot(delivery) => {
                self.mobile_modems.receive(delivery, clock);
                self.observe_latest_snapshot();
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: MobileModemsInteraction) -> Task<MobileModemsMessage> {
        match interaction {
            MobileModemsInteraction::ToggleModem(modem_path) => {
                logged_failure_task(format!("toggle modem {modem_path}"), async move {
                    toggle_mobile_modem(&modem_path).await
                })
            }
            MobileModemsInteraction::ToggleAllRadios => {
                logged_failure_task("toggle all mobile radios".to_owned(), toggle_mobile_radio())
            }
            MobileModemsInteraction::Expansion {
                modem_path,
                expansion,
            } => {
                self.expansions.update(&modem_path, expansion);
                Task::none()
            }
        }
    }

    fn observe_latest_snapshot(&mut self) {
        let Some(mobile_modems) = self.mobile_modems.get() else {
            self.expansions.clear();
            return;
        };
        self.expansions.track(
            mobile_modems
                .modems
                .iter()
                .map(|modem| modem.modem_path.clone()),
        );
    }

    fn any_radio_power(&self) -> RadioPower {
        let any_radio_on = self.mobile_modems.get().is_some_and(|mobile_modems| {
            mobile_modems
                .modems
                .iter()
                .any(|modem| modem.radio_power() == RadioPower::On)
        });
        if any_radio_on {
            RadioPower::On
        } else {
            RadioPower::Off
        }
    }

    fn modem_address_hover_card(&self, modem: &Mobile, family: AddressFamily) -> Vec<Section> {
        let Some(address) = modem_family_address(modem, family) else {
            return Vec::new();
        };
        self.mobile_modems.hover_card_with(vec![Section::new(
            "Address",
            dictionary_table([LabeledValue::new("Address", address)]),
        )])
    }

    fn all_radios_hover_card(&self, radio_power: RadioPower) -> Vec<Section> {
        self.mobile_modems
            .hover_card_with(AllMobileRadiosSwitch { radio_power }.hover_card())
    }

    fn modem_connection_hover_card(&self, modem: &Mobile) -> Vec<Section> {
        self.mobile_modems
            .hover_card_with(ModemConnectionSwitch { modem }.hover_card())
    }

    fn modem_group<'a>(
        &'a self,
        modem: &'a Mobile,
        group_title: WidgetGroupTitle,
    ) -> Option<WidgetGroupContent<'a, MobileModemsInteraction>> {
        let expansion = self.expansions.get(&modem.modem_path)?;
        let modem_path = || modem.modem_path.clone();
        let signal_item = GroupItem::reading(ModemSignalReading { modem }.reading_row())
            .with_hover_card(Box::new(move |_| {
                self.mobile_modems
                    .hover_card_with(ModemSignalReading { modem }.hover_card())
            }));
        let connection_item = (modem.radio_power() == RadioPower::On).then(|| {
            let connection_switch = ModemConnectionSwitch { modem };
            let connection_press_does = connection_switch.press_does();
            GroupItem::reading(connection_switch.reading_row())
                .with_hover_card(Box::new(move |_| self.modem_connection_hover_card(modem)))
                .on_switch_press(
                    modem.link_state(),
                    connection_press_does,
                    MobileModemsInteraction::ToggleModem(modem_path()),
                )
        });
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|family| {
                modem.link_state() == LinkPhase::Up
                    && modem_family_address(modem, *family).is_some()
            })
            .map(|family| {
                GroupItem::reading(InterfaceAddressReading { family }.reading_row())
                    .with_hover_card(Box::new(move |_| {
                        self.modem_address_hover_card(modem, family)
                    }))
            });
        let data_item = (modem.rx.is_some() || modem.tx.is_some()).then(|| {
            GroupItem::reading(ModemDataReading { modem }.reading_row()).with_hover_card(Box::new(
                move |_| {
                    self.mobile_modems
                        .hover_card_with(ModemDataReading { modem }.hover_card())
                },
            ))
        });
        let any_radio_power = self.any_radio_power();
        let radios_switch = AllMobileRadiosSwitch {
            radio_power: any_radio_power,
        };
        let radios_press_does = radios_switch.press_does();
        let radios_item = GroupItem::reading(radios_switch.reading_row())
            .on_switch_press(
                any_radio_power,
                radios_press_does,
                MobileModemsInteraction::ToggleAllRadios,
            )
            .with_hover_card(Box::new(move |_| {
                self.all_radios_hover_card(any_radio_power)
            }));
        let expanded_modem_path = modem_path();
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::MobileModems, modem_path()),
            group_title,
            expansion,
            move |expansion| MobileModemsInteraction::Expansion {
                modem_path: expanded_modem_path.clone(),
                expansion,
            },
            [signal_item]
                .into_iter()
                .chain(connection_item)
                .chain(address_items)
                .chain(data_item)
                .chain([radios_item]),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, MobileModemsInteraction>> {
        self.mobile_modems
            .get()
            .map(|mobile_modems| mobile_modems.modems.as_slice())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(position, modem)| {
                self.modem_group(
                    modem,
                    WidgetGroupTitle::numbered_from_second(
                        WidgetGroupFamily::MobileModems.title(),
                        position,
                    ),
                )
            })
            .collect()
    }
}
