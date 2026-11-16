use std::{borrow::Cow, collections::BTreeMap, mem::take, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    microvm_guests::press_guest_power_button, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::ContainerRunState,
    microvm_guests::{
        MicrovmGuest, MicrovmGuestListing, MicrovmProbe, MicrovmUnit, MicrovmUnits, api_socket_of,
        hypervisor_command_lines::MicrovmHypervisor,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope, unit_active_state::UnitActivity},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage, activity_text},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        process_usage_cells::{approximate_memory_text, uptime_entry},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
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

struct MicrovmUnitIndicator<'a> {
    unit: &'a MicrovmUnit,
    activity: UnitActivity,
}

impl<'a> MicrovmUnitIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        let state = match self.activity {
            UnitActivity::Running => IndicatorState::On,
            UnitActivity::Starting | UnitActivity::Stopping => IndicatorState::Pending,
            UnitActivity::Stopped => IndicatorState::Off,
        };
        GroupIndicator::glyph_only(
            self.unit.instance_name.clone(),
            Phosphor::Lightning(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(activity_text(self.activity).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("State", activity_text(self.activity)),
                LabeledValue::new("Unit", self.unit.unit_name.clone()),
            ]),
        )]
    }
}

struct MicrovmGuestListingIndicator<'a> {
    listing: &'a MicrovmGuestListing,
}

impl<'a> MicrovmGuestListingIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            self.listing.machine_name.clone(),
            Phosphor::Lightning(Weight::Regular).into(),
            ContainerRunState::Running.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(ContainerRunState::Running.label().to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(|_| Self::hover_card()))
    }

    fn hover_card() -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new(
                "State",
                ContainerRunState::Running.label(),
            )]),
        )]
    }
}

struct MicrovmGuestIndicator<'a> {
    machine: &'a MicrovmGuest,
}

impl MicrovmGuestIndicator<'_> {
    fn detail_entries(&self, clock: Clock) -> Vec<LabeledValue> {
        let machine = self.machine;
        let mut detail_entries = vec![
            LabeledValue::new("Hypervisor", machine.hypervisor.product_name()),
            LabeledValue::new("Memory use", approximate_memory_text(machine.memory_in_use)),
        ];
        if let Some(guest_memory) = machine.launch.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let Some(guest_cores) = machine.launch.guest_cores {
            detail_entries.push(LabeledValue::new("Cores", guest_cores.to_string()));
        }
        detail_entries.extend(uptime_entry(clock, machine.started_at));
        if let Some(unit_name) = &machine.unit_name {
            detail_entries.push(LabeledValue::new("Unit", unit_name.clone()));
        }
        if let Some(kernel) = &machine.launch.kernel {
            detail_entries.push(LabeledValue::new("Kernel", kernel.display().to_string()));
        }
        if let Some(api_socket) = &machine.launch.api_socket {
            detail_entries.push(LabeledValue::new(
                "API socket",
                api_socket.display().to_string(),
            ));
        }
        detail_entries.push(LabeledValue::new("PID", machine.listing.pid.to_string()));
        detail_entries
    }
}

impl<'a> MicrovmGuestIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            self.machine.listing.machine_name.clone(),
            Phosphor::Lightning(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(approximate_memory_text(self.machine.memory_in_use)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Machine",
            dictionary_table(self.detail_entries(clock)),
        )]
    }
}

#[derive(Default)]
pub struct MicrovmGuestsGroup {
    probe: SubscribedSnapshot<MicrovmProbe>,
    listings: SubscribedSnapshot<FamilyListings<MicrovmGuestListing>>,
    detail: SubscribedSnapshot<FamilyDetails<MicrovmGuest>>,
    units: SubscribedSnapshot<MicrovmUnits>,
    unit_switches: BTreeMap<String, UnitSwitch>,
    expansion: WidgetGroupExpansion,
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
    Expansion(WidgetGroupExpansionMessage),
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
        let demand = demand.expanded_when(self.expansion.state());
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
                .while_summary_shown(snapshot_subscription::<MicrovmUnits>(buses.clone()))
                .map(MicrovmGuestsMessage::Units),
            demand.while_indicators_shown(unit_switch_subscriptions),
            snapshot_subscription::<MicrovmProbe>(()).map(MicrovmGuestsMessage::Probe),
            demand
                .while_summary_shown(
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

    fn running_guest_of(&self, unit: &MicrovmUnit) -> Option<&MicrovmGuest> {
        self.detail
            .get()?
            .details
            .iter()
            .find(|machine| machine.unit_name.as_deref() == Some(unit.unit_name.as_str()))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_> {
        let rows = self.machine_indicators();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MicrovmGuests,
            GroupSummary::glyph(
                Phosphor::Feather(Weight::Regular).into(),
                self.shown_state(),
            ),
            &self.expansion,
            MicrovmGuestsInteraction::Expansion,
            rows,
        )
    }

    fn machine_indicators(&self) -> Vec<GroupIndicator<'_>> {
        let units = self.unit_listing();
        let mut rows: Vec<GroupIndicator<'_>> =
            units.iter().map(|unit| self.unit_row(unit)).collect();
        rows.extend(self.unmanaged_guest_indicators(units));
        rows
    }

    fn unit_row<'a>(&'a self, unit: &'a MicrovmUnit) -> GroupIndicator<'a> {
        let row = match self.running_guest_of(unit) {
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

    fn unmanaged_guest_indicators<'a>(
        &'a self,
        units: &'a [MicrovmUnit],
    ) -> Vec<GroupIndicator<'a>> {
        let managed_by_unit = |unit_name: Option<&str>| {
            units
                .iter()
                .any(|unit| Some(unit.unit_name.as_str()) == unit_name)
        };
        if let Some(detail) = self.detail.get() {
            return detail
                .details
                .iter()
                .filter(|machine| !managed_by_unit(machine.unit_name.as_deref()))
                .map(|machine| {
                    let row = MicrovmGuestIndicator { machine }
                        .indicator()
                        .with_hover_card(Box::new(move |clock| {
                            self.detail.hover_card_with(
                                MicrovmGuestIndicator { machine }.hover_card(clock),
                            )
                        }));
                    match api_socket_of(machine) {
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
                .collect();
        }
        self.listings
            .get()
            .map(|listings| {
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
            })
            .unwrap_or_default()
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
