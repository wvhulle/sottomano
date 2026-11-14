use std::borrow::Cow;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::virtualbox_machines::press_virtualbox_power_button;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    virtualbox_machines::{
        VirtualBoxFrontend, VirtualBoxMachine, VirtualBoxMachineListing, VirtualBoxMachineListings,
        VirtualBoxMachineProbe, VirtualBoxMachines,
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState, toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::{approximate_memory_text, uptime_entry},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        simple_icons::SimpleIcon,
        value_cells::LabeledValue,
    },
};

const POWER_BUTTON_HINT: &str = "press the guest's power button";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualBoxMachineTarget {
    pid: i32,
}

impl VirtualBoxMachineTarget {
    #[must_use]
    pub const fn of_pid(pid: i32) -> Self {
        Self { pid }
    }
}

impl From<VirtualBoxMachineTarget> for HoverTarget {
    fn from(target: VirtualBoxMachineTarget) -> Self {
        HoverTarget::VirtualBoxMachine(target)
    }
}

struct VirtualBoxMachineListingRow<'a> {
    listing: &'a VirtualBoxMachineListing,
}

impl GroupItemReading for VirtualBoxMachineListingRow<'_> {
    fn name(&self) -> &'static str {
        "VirtualBox machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::VirtualBox)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("running".to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("State", "running"),
            LabeledValue::new("Machine", self.listing.machine_reference.clone()),
            LabeledValue::new("PID", self.listing.pid.to_string()),
        ])
    }
}

struct VirtualBoxMachineRow<'a> {
    machine: &'a VirtualBoxMachine,
}

impl GroupItemReading for VirtualBoxMachineRow<'_> {
    fn name(&self) -> &'static str {
        "VirtualBox machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.machine.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::VirtualBox)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(approximate_memory_text(self.machine.memory_in_use))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let machine = self.machine;
        let mut detail_entries = vec![
            LabeledValue::new(
                "Window",
                match machine.frontend {
                    VirtualBoxFrontend::Headless => "headless",
                    VirtualBoxFrontend::Windowed => "own window",
                },
            ),
            LabeledValue::new("Memory use", approximate_memory_text(machine.memory_in_use)),
        ];
        if let Some(guest_memory) = machine.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let Some(guest_cores) = machine.guest_cores {
            detail_entries.push(LabeledValue::new("Cores", guest_cores.to_string()));
        }
        detail_entries.extend(uptime_entry(clock, machine.started_at));
        if let Some(settings_file) = &machine.settings_file {
            detail_entries.push(LabeledValue::new(
                "Settings",
                settings_file.display().to_string(),
            ));
        }
        detail_entries.extend([
            LabeledValue::new("Machine", machine.machine_reference.clone()),
            LabeledValue::new("PID", machine.pid.to_string()),
        ]);
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Default)]
pub struct VirtualBoxMachinesGroup {
    probe: LatestSnapshot<VirtualBoxMachineProbe, HidesFetchFailures>,
    listings: LatestSnapshot<VirtualBoxMachineListings, HidesFetchFailures>,
    detail: LatestSnapshot<VirtualBoxMachines, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum VirtualBoxMachinesMessage {
    Probe(SnapshotDelivery<VirtualBoxMachineProbe>),
    Listings(SnapshotDelivery<VirtualBoxMachineListings>),
    Machines(SnapshotDelivery<VirtualBoxMachines>),
    PowerButtonRefused(String),
    Interaction(VirtualBoxMachinesInteraction),
}

#[derive(Debug, Clone)]
pub enum VirtualBoxMachinesInteraction {
    PressPowerButton(String),
    Expansion(WidgetGroupExpansionMessage),
}

impl VirtualBoxMachinesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        demand: GroupSourceDemand,
    ) -> Subscription<VirtualBoxMachinesMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<VirtualBoxMachineProbe>(())
                .map(VirtualBoxMachinesMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<VirtualBoxMachineListings>(()))
                .map(VirtualBoxMachinesMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<VirtualBoxMachines>(()))
                .map(VirtualBoxMachinesMessage::Machines),
        ])
    }

    pub fn update(
        &mut self,
        message: VirtualBoxMachinesMessage,
        clock: Clock,
    ) -> Task<VirtualBoxMachinesMessage> {
        match message {
            VirtualBoxMachinesMessage::Probe(delivery) => {
                self.probe.receive(delivery, clock);
                Task::none()
            }
            VirtualBoxMachinesMessage::Listings(delivery) => {
                self.listings.receive(delivery, clock);
                Task::none()
            }
            VirtualBoxMachinesMessage::Machines(delivery) => {
                self.detail.receive(delivery, clock);
                Task::none()
            }
            VirtualBoxMachinesMessage::PowerButtonRefused(refusal) => {
                tracing::warn!("virtualbox power button: {refusal}");
                Task::none()
            }
            VirtualBoxMachinesMessage::Interaction(
                VirtualBoxMachinesInteraction::PressPowerButton(machine_reference),
            ) => Task::future(async move {
                press_virtualbox_power_button(&machine_reference)
                    .await
                    .err()
            })
            .and_then(|refusal| Task::done(VirtualBoxMachinesMessage::PowerButtonRefused(refusal))),
            VirtualBoxMachinesMessage::Interaction(VirtualBoxMachinesInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: &VirtualBoxMachineTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        if let Some(detail) = self.detail.get() {
            let machine = detail
                .machines
                .iter()
                .find(|machine| machine.pid == target.pid)?;
            return self
                .detail
                .hover_card_with(VirtualBoxMachineRow { machine }.hover_card(clock));
        }
        let listing = self
            .listings
            .get()?
            .listings
            .iter()
            .find(|listing| listing.pid == target.pid)?;
        self.listings
            .hover_card_with(VirtualBoxMachineListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, VirtualBoxMachinesInteraction> {
        let rows = self.machine_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::VirtualBoxMachines,
            &self.expansion,
            VirtualBoxMachinesInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: IconGlyph::SimpleIcon(SimpleIcon::VirtualBox),
            reading: self,
        })
    }

    fn machine_rows(&self) -> Vec<GroupItem<'_, VirtualBoxMachinesInteraction>> {
        if let Some(detail) = self.detail.get() {
            return detail
                .machines
                .iter()
                .map(|machine| {
                    GroupItem::reading(
                        VirtualBoxMachineRow { machine },
                        VirtualBoxMachineTarget::of_pid(machine.pid),
                    )
                    .on_switch_press(
                        SwitchPosition::On,
                        POWER_BUTTON_HINT,
                        VirtualBoxMachinesInteraction::PressPowerButton(
                            machine.machine_reference.clone(),
                        ),
                    )
                })
                .collect();
        }
        self.listings
            .get()
            .map(|listings| {
                listings
                    .listings
                    .iter()
                    .map(|listing| {
                        GroupItem::reading(
                            VirtualBoxMachineListingRow { listing },
                            VirtualBoxMachineTarget::of_pid(listing.pid),
                        )
                        .on_switch_press(
                            SwitchPosition::On,
                            POWER_BUTTON_HINT,
                            VirtualBoxMachinesInteraction::PressPowerButton(
                                listing.machine_reference.clone(),
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl GroupItemReading for VirtualBoxMachinesGroup {
    fn name(&self) -> &'static str {
        "VirtualBox machines"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "VirtualBox machines".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::VirtualBox)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe
            .get()
            .map(|probe| format!("{} running", probe.running_count))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.probe
            .get()
            .filter(|probe| probe.running_count > 0)
            .map_or(ReadingDisplayState::Off, |_| ReadingDisplayState::On)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            self.probe
                .get()
                .map(|probe| {
                    vec![LabeledValue::new(
                        "Running",
                        probe.running_count.to_string(),
                    )]
                })
                .unwrap_or_default(),
        )
    }
}
