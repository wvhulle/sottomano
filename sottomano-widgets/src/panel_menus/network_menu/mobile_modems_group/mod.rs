use std::iter;

use crate::group_indicators::indicator_group_content::IndicatorListing;
mod all_radios_indicator;
mod modem_address_indicator;
mod modem_connection_indicator;
mod modem_data_indicator;
mod modem_signal_indicator;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::{
    toggle_mobile_modem, toggle_mobile_radio,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::modem_manager_mobile::{Mobile, MobileModems},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansionMessage, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
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
        expansion: IndicatorGroupExpansionMessage,
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

    fn modem_group<'view>(
        &'view self,
        modem: &'view Mobile,
        group_title: IndicatorGroupTitle,
    ) -> Option<IndicatorGroupContent<'view>> {
        let expansion = self.expansions.expansions.get(&modem.modem_path)?;
        let modem_path = || modem.modem_path.clone();
        let headline_indicator = self
            .modem_signal_row(modem)
            .with_headline_glyph(Phosphor::SimCard(Weight::Regular).into());
        let rows = self
            .modem_connection_rows(modem)
            .into_iter()
            .chain(self.modem_address_rows(modem))
            .chain(self.modem_data_rows(modem))
            .chain(iter::once(self.all_radios_row()));
        let expanded_modem_path = modem_path();
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(IndicatorGroupFamily::MobileModems, modem_path()),
            group_title,
            headline_indicator,
            expansion,
            move |expansion| MobileModemsInteraction::Expansion {
                modem_path: expanded_modem_path.clone(),
                expansion,
            },
            IndicatorListing::Listed(
                iter::once(self.modem_signal_row(modem))
                    .chain(rows)
                    .collect(),
            ),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<IndicatorGroupContent<'_>> {
        self.mobile_modems
            .get()
            .map(|mobile_modems| mobile_modems.modems.as_slice())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(position, modem)| {
                self.modem_group(
                    modem,
                    IndicatorGroupTitle::numbered_from_second(
                        IndicatorGroupFamily::MobileModems.title(),
                        position,
                    ),
                )
            })
            .collect()
    }
}
