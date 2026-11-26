mod microvm_guest_indicators;

use std::{borrow::Cow, collections::BTreeMap, mem::take, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    microvm_guests::press_guest_power_button, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    microvm_guests::{
        EnclosingUnit, MicrovmGuest, MicrovmGuestListing, MicrovmProbe, MicrovmUnit, MicrovmUnits,
        hypervisor_command_lines::MicrovmHypervisor, power_button_socket,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope, unit_active_state::UnitActivity},
};

use self::microvm_guest_indicators::{
    MicrovmGuestIndicator, MicrovmGuestListingIndicator, MicrovmUnitIndicator,
};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const POWER_BUTTON_HINT: &str = "press the guest's power button";
const MICROVM_SWITCH_FACE: UnitSwitchFace = UnitSwitchFace {
    name: Cow::Borrowed("microvm.nix guest"),
    icon: Phosphor::Lightning(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start the microVM",
        turning_off: "stop the microVM",
    },
};

fn managed_by(machine: &MicrovmGuest, unit: &MicrovmUnit) -> bool {
    matches!(&machine.enclosing_unit, EnclosingUnit::Service(unit_name) if *unit_name == unit.unit_name)
}

#[derive(Default)]
pub struct MicrovmGuestsGroup {
    probe: SubscribedSnapshot<MicrovmProbe>,
    listings: SubscribedSnapshot<FamilyListings<MicrovmGuestListing>>,
    detail: SubscribedSnapshot<FamilyDetails<MicrovmGuest>>,
    units: SubscribedSnapshot<MicrovmUnits>,
    unit_switches: BTreeMap<String, UnitSwitch>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum MicrovmGuestsMessage {
    Probe(SnapshotDelivery<MicrovmProbe>),
    Listings(SnapshotDelivery<FamilyListings<MicrovmGuestListing>>),
    Machines(SnapshotDelivery<FamilyDetails<MicrovmGuest>>),
    Units(SnapshotDelivery<MicrovmUnits>),
    Switch(String, UnitSwitchMessage),
    PowerButtonRefused(GuestPowerError),
}

#[derive(Debug, Clone)]
pub enum MicrovmGuestsInteraction {
    PressPowerButton(MicrovmHypervisor, PathBuf),
    UnitSwitchPressed(String),
    Expansion(IndicatorGroupExpansionMessage),
}

impl MicrovmGuestsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        demand: GroupSourceDemand,
        buses: &DesktopBuses,
    ) -> Subscription<MicrovmGuestsMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        let unit_switch_subscriptions = Subscription::batch(self.unit_switches.iter().map(
            |(instance_name, unit_switch)| {
                unit_switch
                    .subscription(buses)
                    .with(instance_name.clone())
                    .map(|(instance_name, switch_message)| {
                        MicrovmGuestsMessage::Switch(instance_name, switch_message)
                    })
            },
        ));
        Subscription::batch([
            demand
                .while_headline_shown(snapshot_subscription::<MicrovmUnits>(buses.clone()))
                .map(MicrovmGuestsMessage::Units),
            demand.while_indicators_shown(unit_switch_subscriptions),
            snapshot_subscription::<MicrovmProbe>(()).map(MicrovmGuestsMessage::Probe),
            demand
                .while_headline_shown(
                    snapshot_subscription::<FamilyListings<MicrovmGuestListing>>(()),
                )
                .map(MicrovmGuestsMessage::Listings),
            demand
                .while_indicators_shown(snapshot_subscription::<FamilyDetails<MicrovmGuest>>(()))
                .map(MicrovmGuestsMessage::Machines),
        ])
    }

    pub fn update(
        &mut self,
        message: MicrovmGuestsMessage,
        clock: Clock,
    ) -> Task<MicrovmGuestsMessage> {
        match message {
            MicrovmGuestsMessage::Probe(delivery) => {
                self.probe.receive(delivery, clock);
                Task::none()
            }
            MicrovmGuestsMessage::Listings(delivery) => {
                self.listings.receive(delivery, clock);
                Task::none()
            }
            MicrovmGuestsMessage::Machines(delivery) => {
                self.detail.receive(delivery, clock);
                Task::none()
            }
            MicrovmGuestsMessage::Units(delivery) => {
                self.units.receive(delivery, clock);
                self.adopt_unit_switches();
                Task::none()
            }
            MicrovmGuestsMessage::Switch(instance_name, switch_message) => self
                .unit_switch_task(instance_name, |unit_switch| {
                    unit_switch.update(switch_message, clock)
                }),
            MicrovmGuestsMessage::PowerButtonRefused(refusal) => {
                tracing::warn!("microvm power button: {refusal}");
                Task::none()
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: MicrovmGuestsInteraction,
    ) -> Task<MicrovmGuestsMessage> {
        match interaction {
            MicrovmGuestsInteraction::UnitSwitchPressed(instance_name) => {
                self.unit_switch_task(instance_name, UnitSwitch::press)
            }
            MicrovmGuestsInteraction::PressPowerButton(hypervisor, api_socket) => {
                Task::future(async move {
                    press_guest_power_button(hypervisor, &api_socket)
                        .await
                        .err()
                })
                .and_then(|refusal| Task::done(MicrovmGuestsMessage::PowerButtonRefused(refusal)))
            }
            MicrovmGuestsInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn adopt_unit_switches(&mut self) {
        let mut previous_switches = take(&mut self.unit_switches);
        self.unit_switches = self
            .units
            .get()
            .into_iter()
            .flat_map(|units| units.units.iter())
            .map(|unit| {
                let unit_switch = previous_switches
                    .remove(&unit.instance_name)
                    .unwrap_or_else(|| {
                        UnitSwitch::new(
                            ScopedSystemdUnit {
                                scope: SystemdUnitScope::SystemWide,
                                unit_name: unit.unit_name.clone(),
                            },
                            MICROVM_SWITCH_FACE,
                        )
                    });
                (unit.instance_name.clone(), unit_switch)
            })
            .collect();
    }

    fn unit_switch_task(
        &mut self,
        instance_name: String,
        change: impl FnOnce(&mut UnitSwitch) -> Task<UnitSwitchMessage>,
    ) -> Task<MicrovmGuestsMessage> {
        let Some(unit_switch) = self.unit_switches.get_mut(&instance_name) else {
            return Task::none();
        };
        change(unit_switch).map(move |switch_message| {
            MicrovmGuestsMessage::Switch(instance_name.clone(), switch_message)
        })
    }

    fn unit_listing(&self) -> &[MicrovmUnit] {
        self.units
            .get()
            .map(|units| units.units.as_slice())
            .unwrap_or_default()
    }

    fn unit_activity(&self, unit: &MicrovmUnit) -> UnitActivity {
        self.unit_switches
            .get(&unit.instance_name)
            .and_then(UnitSwitch::activity)
            .unwrap_or(UnitActivity::Stopped)
    }

    fn managed_running_guest(&self, unit: &MicrovmUnit) -> Option<&MicrovmGuest> {
        self.detail
            .get()?
            .details
            .iter()
            .find(|machine| managed_by(machine, unit))
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::MicrovmGuests,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::MicrovmGuests.title(),
                microvm_glyph(),
                self.shown_state(),
            ),
            &self.expansion,
            MicrovmGuestsInteraction::Expansion,
            self.indicator_listing(),
        )
    }

    fn indicator_listing(&self) -> IndicatorListing<'_> {
        [
            self.units.indicator_listing(
                |units| units.units.iter().map(|unit| self.unit_row(unit)).collect(),
                microvm_glyph(),
            ),
            self.unmanaged_guest_listing(self.unit_listing()),
        ]
        .into_iter()
        .collect()
    }

    fn unit_row<'view>(&'view self, unit: &'view MicrovmUnit) -> GroupIndicator<'view> {
        let row = match self.managed_running_guest(unit) {
            Some(machine) => MicrovmGuestIndicator { machine }
                .indicator()
                .with_hover_card(Box::new(move |clock| {
                    self.detail
                        .hover_card_with(MicrovmGuestIndicator { machine }.hover_card(clock))
                })),
            None => MicrovmUnitIndicator {
                unit,
                activity: self.unit_activity(unit),
            }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                let activity = self.unit_activity(unit);
                self.units
                    .hover_card_with(MicrovmUnitIndicator { unit, activity }.hover_card())
            })),
        };
        match self.unit_switches.get(&unit.instance_name) {
            Some(unit_switch) => unit_switch.attach_switch_press(
                row,
                MicrovmGuestsInteraction::UnitSwitchPressed(unit.instance_name.clone()),
            ),
            None => row,
        }
    }

    fn unmanaged_guest_listing<'view>(
        &'view self,
        units: &'view [MicrovmUnit],
    ) -> IndicatorListing<'view> {
        let managed_by_unit =
            |machine: &MicrovmGuest| units.iter().any(|unit| managed_by(machine, unit));
        if let Some(detail) = self.detail.get() {
            return IndicatorListing::Listed(
                detail
                    .details
                    .iter()
                    .filter(|machine| !managed_by_unit(machine))
                    .map(|machine| {
                        let row = MicrovmGuestIndicator { machine }
                            .indicator()
                            .with_hover_card(Box::new(move |clock| {
                                self.detail.hover_card_with(
                                    MicrovmGuestIndicator { machine }.hover_card(clock),
                                )
                            }));
                        match power_button_socket(machine) {
                            Some(api_socket) => row.on_stop_press(
                                POWER_BUTTON_HINT,
                                MicrovmGuestsInteraction::PressPowerButton(
                                    machine.hypervisor,
                                    api_socket.clone(),
                                ),
                            ),
                            None => row.inert_switch(SwitchPosition::On),
                        }
                    })
                    .collect(),
            );
        }
        self.listings.indicator_listing(
            |listings| {
                listings
                    .listings
                    .iter()
                    .filter(|listing| {
                        !units
                            .iter()
                            .any(|unit| unit.instance_name == listing.machine_name)
                    })
                    .map(|listing| {
                        MicrovmGuestListingIndicator { listing }
                            .indicator()
                            .with_hover_card(Box::new(move |_| {
                                self.listings
                                    .hover_card_with(MicrovmGuestListingIndicator::hover_card())
                            }))
                    })
                    .collect()
            },
            microvm_glyph(),
        )
    }
}

impl MicrovmGuestsGroup {
    fn shown_state(&self) -> IndicatorState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.running_count > 0)
        {
            IndicatorState::On
        } else {
            IndicatorState::Off
        }
    }
}

fn microvm_glyph() -> IconGlyph {
    Phosphor::Feather(Weight::Regular).into()
}
