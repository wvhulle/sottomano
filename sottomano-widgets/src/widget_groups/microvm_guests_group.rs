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
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage, activity_text},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, dictionary_table},
    },
    widget_groups::{
        process_usage_cells::{approximate_memory_text, uptime_entry},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicrovmGuestTarget {
    RunningProcess { pid: i32 },
    Unit { instance_name: Cow<'static, str> },
}

struct MicrovmUnitRow<'a> {
    unit: &'a MicrovmUnit,
    activity: UnitActivity,
}

impl<'a> MicrovmUnitRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let state = match self.activity {
            UnitActivity::Running => ReadingDisplayState::On,
            UnitActivity::Starting | UnitActivity::Stopping => ReadingDisplayState::Pending,
            UnitActivity::Stopped => ReadingDisplayState::Off,
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(activity_text(self.activity).to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                self.unit.instance_name.clone(),
                Phosphor::Lightning(Weight::Regular).into(),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
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

impl From<MicrovmGuestTarget> for HoverTarget {
    fn from(target: MicrovmGuestTarget) -> Self {
        HoverTarget::MicrovmGuest(target)
    }
}

struct MicrovmGuestListingRow<'a> {
    listing: &'a MicrovmGuestListing,
}

impl<'a> MicrovmGuestListingRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(ContainerRunState::Running.label().to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                self.listing.machine_name.clone(),
                Phosphor::Lightning(Weight::Regular).into(),
                ContainerRunState::Running.into(),
                Box::new(|_| Self::hover_card()),
            )
        }
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

struct MicrovmGuestRow<'a> {
    machine: &'a MicrovmGuest,
}

impl MicrovmGuestRow<'_> {
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

impl<'a> MicrovmGuestRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(approximate_memory_text(self.machine.memory_in_use)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                self.machine.listing.machine_name.clone(),
                Phosphor::Lightning(Weight::Regular).into(),
                ReadingDisplayState::On,
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
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
    Interaction(MicrovmGuestsInteraction),
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
                .while_face_shown(snapshot_subscription::<MicrovmUnits>(buses.clone()))
                .map(MicrovmGuestsMessage::Units),
            demand.while_rows_shown(unit_switch_subscriptions),
            snapshot_subscription::<MicrovmProbe>(()).map(MicrovmGuestsMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<FamilyListings<MicrovmGuestListing>>(()))
                .map(MicrovmGuestsMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<FamilyDetails<MicrovmGuest>>(()))
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
            MicrovmGuestsMessage::Interaction(MicrovmGuestsInteraction::UnitSwitchPressed(
                instance_name,
            )) => self.unit_switch_task(instance_name, UnitSwitch::press),
            MicrovmGuestsMessage::PowerButtonRefused(refusal) => {
                tracing::warn!("microvm power button: {refusal}");
                Task::none()
            }
            MicrovmGuestsMessage::Interaction(MicrovmGuestsInteraction::PressPowerButton(
                hypervisor,
                api_socket,
            )) => Task::future(async move {
                press_guest_power_button(hypervisor, &api_socket)
                    .await
                    .err()
            })
            .and_then(|refusal| Task::done(MicrovmGuestsMessage::PowerButtonRefused(refusal))),
            MicrovmGuestsMessage::Interaction(MicrovmGuestsInteraction::Expansion(
                expansion_message,
            )) => {
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

    pub fn group_content(&self) -> WidgetGroupContent<'_, MicrovmGuestsInteraction> {
        let rows = self.machine_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MicrovmGuests,
            &self.expansion,
            MicrovmGuestsInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::Lightning(Weight::Regular).into(),
            state: self.shown_state(),
        })
    }

    fn machine_rows(&self) -> Vec<GroupItem<'_, MicrovmGuestsInteraction>> {
        let units = self.unit_listing();
        let mut rows: Vec<GroupItem<'_, MicrovmGuestsInteraction>> =
            units.iter().map(|unit| self.unit_row(unit)).collect();
        rows.extend(self.unmanaged_guest_rows(units));
        rows
    }

    fn unit_row<'a>(&'a self, unit: &'a MicrovmUnit) -> GroupItem<'a, MicrovmGuestsInteraction> {
        let row = match self.running_guest_of(unit) {
            Some(machine) => GroupItem::reading(
                MicrovmGuestRow { machine }.reading_row(),
                MicrovmGuestTarget::RunningProcess {
                    pid: machine.listing.pid,
                },
            )
            .with_hover_card(Box::new(move |clock| {
                self.detail
                    .hover_card_with(MicrovmGuestRow { machine }.hover_card(clock))
            })),
            None => GroupItem::reading(
                MicrovmUnitRow {
                    unit,
                    activity: self.unit_activity(unit),
                }
                .reading_row(),
                MicrovmGuestTarget::Unit {
                    instance_name: Cow::Owned(unit.instance_name.clone()),
                },
            )
            .with_hover_card(Box::new(move |_| {
                let activity = self.unit_activity(unit);
                self.units
                    .hover_card_with(MicrovmUnitRow { unit, activity }.hover_card())
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

    fn unmanaged_guest_rows<'a>(
        &'a self,
        units: &'a [MicrovmUnit],
    ) -> Vec<GroupItem<'a, MicrovmGuestsInteraction>> {
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
                    let row = GroupItem::reading(
                        MicrovmGuestRow { machine }.reading_row(),
                        MicrovmGuestTarget::RunningProcess {
                            pid: machine.listing.pid,
                        },
                    )
                    .with_hover_card(Box::new(move |clock| {
                        self.detail
                            .hover_card_with(MicrovmGuestRow { machine }.hover_card(clock))
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
                        GroupItem::reading(
                            MicrovmGuestListingRow { listing }.reading_row(),
                            MicrovmGuestTarget::RunningProcess { pid: listing.pid },
                        )
                        .with_hover_card(Box::new(move |_| {
                            self.listings
                                .hover_card_with(MicrovmGuestListingRow::hover_card())
                        }))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl MicrovmGuestsGroup {
    fn shown_state(&self) -> ReadingDisplayState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.running_count > 0)
        {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }
}
