mod modem_signal_reading;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::{
    toggle_mobile_modem, toggle_mobile_radio,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        modem_manager_mobile::{Mobile, MobileModems},
        network_manager_proxies::{DeviceLinkState, RadioPower},
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
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        interface_address_sections::{AddressFamily, InterfaceAddressReading},
        widget_group_content::{KeyedExpansions, WidgetGroupContent, WidgetGroupExpansionMessage},
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MobileModemTarget {
    Signal {
        modem_path: String,
    },
    Connection {
        modem_path: String,
    },
    Address {
        modem_path: String,
        family: AddressFamily,
    },
    Data {
        modem_path: String,
    },
    AllRadios,
}

impl From<MobileModemTarget> for HoverTarget {
    fn from(target: MobileModemTarget) -> Self {
        HoverTarget::MobileModem(target)
    }
}

#[derive(Debug, Clone)]
pub enum MobileModemsMessage {
    Snapshot(SnapshotDelivery<MobileModems>),
    ToggleModem(String),
    ToggleAllRadios,
    Expansion {
        modem_path: String,
        expansion: WidgetGroupExpansionMessage,
    },
}

#[derive(Default)]
pub struct MobileModemsGroup {
    mobile_modems: LatestSnapshot<MobileModems, HidesFetchFailures>,
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
            MobileModemsMessage::ToggleModem(modem_path) => {
                return logged_failure_task(format!("toggle modem {modem_path}"), async move {
                    toggle_mobile_modem(&modem_path).await
                });
            }
            MobileModemsMessage::ToggleAllRadios => {
                return logged_failure_task(
                    "toggle all mobile radios".to_owned(),
                    toggle_mobile_radio(),
                );
            }
            MobileModemsMessage::Expansion {
                modem_path,
                expansion,
            } => {
                self.expansions.update(&modem_path, expansion);
            }
        }
        Task::none()
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

    fn modem_at(&self, modem_path: &str) -> Option<&Mobile> {
        self.mobile_modems
            .get()?
            .modems
            .iter()
            .find(|modem| modem.modem_path == modem_path)
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

    #[must_use]
    pub fn hover_card_for(&self, target: &MobileModemTarget, clock: Clock) -> Option<HoverCard> {
        let hover_card_body = match target {
            MobileModemTarget::Signal { modem_path } => ModemSignalReading {
                modem: self.modem_at(modem_path)?,
            }
            .hover_card(clock),
            MobileModemTarget::Connection { modem_path } => ModemConnectionSwitch {
                modem: self.modem_at(modem_path)?,
            }
            .hover_card(clock),
            MobileModemTarget::Address { modem_path, family } => {
                let address = modem_family_address(self.modem_at(modem_path)?, *family)?;
                HoverCardBody::dictionary([LabeledValue::new("Address", address)])
            }
            MobileModemTarget::Data { modem_path } => ModemDataReading {
                modem: self.modem_at(modem_path)?,
            }
            .hover_card(clock),
            MobileModemTarget::AllRadios => AllMobileRadiosSwitch {
                radio_power: self.any_radio_power(),
            }
            .hover_card(clock),
        };
        self.mobile_modems.hover_card_with(hover_card_body)
    }

    fn modem_group<'a>(
        &'a self,
        modem: &'a Mobile,
        group_title: WidgetGroupTitle,
    ) -> Option<WidgetGroupContent<'a, MobileModemsMessage>> {
        let expansion = self.expansions.get(&modem.modem_path)?;
        let modem_path = || modem.modem_path.clone();
        let signal_item = GroupItem::reading(
            ModemSignalReading { modem },
            MobileModemTarget::Signal {
                modem_path: modem_path(),
            },
        );
        let connection_item = (modem.radio_power() == RadioPower::On).then(|| {
            let connection_switch = ModemConnectionSwitch { modem };
            let connection_press_does = connection_switch.press_does();
            GroupItem::reading(
                connection_switch,
                MobileModemTarget::Connection {
                    modem_path: modem_path(),
                },
            )
            .on_switch_press(
                modem.link_state(),
                connection_press_does,
                MobileModemsMessage::ToggleModem(modem_path()),
            )
        });
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|family| {
                modem.link_state() == DeviceLinkState::Connected
                    && modem_family_address(modem, *family).is_some()
            })
            .map(|family| {
                GroupItem::reading(
                    InterfaceAddressReading { family },
                    MobileModemTarget::Address {
                        modem_path: modem_path(),
                        family,
                    },
                )
            });
        let data_item = (modem.rx.is_some() || modem.tx.is_some()).then(|| {
            GroupItem::reading(
                ModemDataReading { modem },
                MobileModemTarget::Data {
                    modem_path: modem_path(),
                },
            )
        });
        let any_radio_power = self.any_radio_power();
        let radios_switch = AllMobileRadiosSwitch {
            radio_power: any_radio_power,
        };
        let radios_press_does = radios_switch.press_does();
        let radios_item = GroupItem::reading(radios_switch, MobileModemTarget::AllRadios)
            .on_switch_press(
                any_radio_power,
                radios_press_does,
                MobileModemsMessage::ToggleAllRadios,
            );
        let expanded_modem_path = modem_path();
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::MobileModems, modem_path()),
            group_title,
            expansion,
            move |expansion| MobileModemsMessage::Expansion {
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
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, MobileModemsMessage>> {
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
