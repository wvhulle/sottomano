use std::{borrow::Cow, collections::BTreeMap, mem::take, path::PathBuf};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::microvm_guests::press_guest_power_button;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    microvm_guests::{
        MicrovmGuest, MicrovmGuestListing, MicrovmGuestListings, MicrovmGuests, MicrovmProbe,
        MicrovmUnit, MicrovmUnits, api_socket_of, hypervisor_command_lines::MicrovmHypervisor,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope, unit_active_state::UnitActivity},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage, activity_text},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::{
            approximate_memory_text, running_display_state, running_state_text, uptime_entry,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const POWER_BUTTON_HINT: &str = "press the guest's power button";
const MICROVM_SWITCH_FACE: UnitSwitchFace = UnitSwitchFace {
    name: "MicroVM",
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

impl MicrovmGuestTarget {
    #[must_use]
    pub const fn of_pid(pid: i32) -> Self {
        Self::RunningProcess { pid }
    }
}

struct MicrovmUnitRow<'a> {
    unit: &'a MicrovmUnit,
    activity: UnitActivity,
}

impl GroupItemReading for MicrovmUnitRow<'_> {
    fn name(&self) -> &'static str {
        "MicroVM"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.unit.instance_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(activity_text(self.activity).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.activity {
            UnitActivity::Running => ReadingDisplayState::On,
            UnitActivity::Starting | UnitActivity::Stopping => ReadingDisplayState::Pending,
            UnitActivity::Stopped => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("State", activity_text(self.activity)),
            LabeledValue::new("Unit", self.unit.unit_name.clone()),
        ])
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

impl GroupItemReading for MicrovmGuestListingRow<'_> {
    fn name(&self) -> &'static str {
        "MicroVM"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(running_state_text(self.listing.running).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        running_display_state(self.listing.running)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new(
            "State",
            running_state_text(self.listing.running),
        )])
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
        detail_entries.push(LabeledValue::new("PID", machine.pid.to_string()));
        detail_entries
    }
}

impl GroupItemReading for MicrovmGuestRow<'_> {
    fn name(&self) -> &'static str {
        "MicroVM"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.machine.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(approximate_memory_text(self.machine.memory_in_use))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(self.detail_entries(clock))
    }
}

#[derive(Default)]
pub struct MicrovmGuestsGroup {
    probe: LatestSnapshot<MicrovmProbe, HidesFetchFailures>,
    listings: LatestSnapshot<MicrovmGuestListings, HidesFetchFailures>,
    detail: LatestSnapshot<MicrovmGuests, HidesFetchFailures>,
    units: LatestSnapshot<MicrovmUnits, HidesFetchFailures>,
    unit_switches: BTreeMap<String, UnitSwitch>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum MicrovmGuestsMessage {
    Probe(SnapshotDelivery<MicrovmProbe>),
    Listings(SnapshotDelivery<MicrovmGuestListings>),
    Machines(SnapshotDelivery<MicrovmGuests>),
    Units(SnapshotDelivery<MicrovmUnits>),
    Switch(String, UnitSwitchMessage),
    PowerButtonRefused(String),
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
                .while_face_shown(snapshot_subscription::<MicrovmGuestListings>(()))
                .map(MicrovmGuestsMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<MicrovmGuests>(()))
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
            .machines
            .iter()
            .find(|machine| machine.unit_name.as_deref() == Some(unit.unit_name.as_str()))
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &MicrovmGuestTarget, clock: Clock) -> Option<HoverCard> {
        let pid = match target {
            MicrovmGuestTarget::RunningProcess { pid } => *pid,
            MicrovmGuestTarget::Unit { instance_name } => {
                let unit = self
                    .unit_listing()
                    .iter()
                    .find(|unit| unit.instance_name == instance_name.as_ref())?;
                let activity = self.unit_activity(unit);
                return self
                    .units
                    .hover_card_with(MicrovmUnitRow { unit, activity }.hover_card(clock));
            }
        };
        if let Some(detail) = self.detail.get() {
            let machine = detail.machines.iter().find(|machine| machine.pid == pid)?;
            return self
                .detail
                .hover_card_with(MicrovmGuestRow { machine }.hover_card(clock));
        }
        let listing = self
            .listings
            .get()?
            .listings
            .iter()
            .find(|listing| listing.pid == pid)?;
        self.listings
            .hover_card_with(MicrovmGuestListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, MicrovmGuestsInteraction> {
        let rows = self.machine_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MicrovmGuests,
            &self.expansion,
            MicrovmGuestsInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::Lightning(Weight::Regular).into(),
            reading: self,
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
                MicrovmGuestRow { machine },
                MicrovmGuestTarget::of_pid(machine.pid),
            ),
            None => GroupItem::reading(
                MicrovmUnitRow {
                    unit,
                    activity: self.unit_activity(unit),
                },
                MicrovmGuestTarget::Unit {
                    instance_name: Cow::Owned(unit.instance_name.clone()),
                },
            ),
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
                .machines
                .iter()
                .filter(|machine| !managed_by_unit(machine.unit_name.as_deref()))
                .map(|machine| {
                    let row = GroupItem::reading(
                        MicrovmGuestRow { machine },
                        MicrovmGuestTarget::of_pid(machine.pid),
                    );
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
                            MicrovmGuestListingRow { listing },
                            MicrovmGuestTarget::of_pid(listing.pid),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl GroupItemReading for MicrovmGuestsGroup {
    fn name(&self) -> &'static str {
        "MicroVMs"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "MicroVMs".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe
            .get()
            .map(|probe| format!("{} running", probe.running_guest_processes))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.running_guest_processes > 0)
        {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            self.probe
                .get()
                .map(|probe| {
                    vec![LabeledValue::new(
                        "Running guests",
                        probe.running_guest_processes.to_string(),
                    )]
                })
                .unwrap_or_default(),
        )
    }
}
