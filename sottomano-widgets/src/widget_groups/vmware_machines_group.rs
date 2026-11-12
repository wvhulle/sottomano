use std::{borrow::Cow, convert::identity};

use chrono::Locale;
use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    vmware_machines::{
        VmwareMachine, VmwareMachineListing, VmwareMachineListings, VmwareMachineProbe,
        VmwareMachines,
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::approximate_memory_text,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmwareMachineTarget {
    pid: i32,
}

impl VmwareMachineTarget {
    #[must_use]
    pub const fn of_pid(pid: i32) -> Self {
        Self { pid }
    }
}

impl From<VmwareMachineTarget> for HoverTarget {
    fn from(target: VmwareMachineTarget) -> Self {
        HoverTarget::VmwareMachine(target)
    }
}

struct VmwareMachineListingRow<'a> {
    listing: &'a VmwareMachineListing,
}

impl GroupItemReading for VmwareMachineListingRow<'_> {
    fn name(&self) -> &'static str {
        "VMware machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
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
            LabeledValue::new("PID", self.listing.pid.to_string()),
        ])
    }
}

struct VmwareMachineRow<'a> {
    machine: &'a VmwareMachine,
}

impl GroupItemReading for VmwareMachineRow<'_> {
    fn name(&self) -> &'static str {
        "VMware machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.machine.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(approximate_memory_text(self.machine.memory_in_use))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let machine = self.machine;
        let mut detail_entries = vec![LabeledValue::new(
            "Memory use",
            approximate_memory_text(machine.memory_in_use),
        )];
        if let Some(guest_os) = &machine.guest_os {
            detail_entries.push(LabeledValue::new("Guest", guest_os.clone()));
        }
        if let Some(guest_memory) = machine.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let Some(guest_cores) = machine.guest_cores {
            detail_entries.push(LabeledValue::new("Cores", guest_cores.to_string()));
        }
        if let Ok(uptime) = clock.wall.duration_since(machine.started_at) {
            detail_entries.push(LabeledValue::new("Uptime", ValueCell::duration(uptime)));
        }
        detail_entries.extend([
            LabeledValue::new(
                "Configuration",
                machine.configuration_file.display().to_string(),
            ),
            LabeledValue::new("PID", machine.pid.to_string()),
        ]);
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Default)]
pub struct VmwareMachinesGroup {
    probe: LatestSnapshot<VmwareMachineProbe, HidesFetchFailures>,
    listings: LatestSnapshot<VmwareMachineListings, HidesFetchFailures>,
    detail: LatestSnapshot<VmwareMachines, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum VmwareMachinesMessage {
    Probe(SnapshotDelivery<VmwareMachineProbe>),
    Listings(SnapshotDelivery<VmwareMachineListings>),
    Machines(SnapshotDelivery<VmwareMachines>),
    Expansion(WidgetGroupExpansionMessage),
}

impl VmwareMachinesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<VmwareMachinesMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<VmwareMachineProbe>(()).map(VmwareMachinesMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<VmwareMachineListings>(()))
                .map(VmwareMachinesMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<VmwareMachines>(()))
                .map(VmwareMachinesMessage::Machines),
        ])
    }

    pub fn update(&mut self, message: VmwareMachinesMessage, clock: Clock) {
        match message {
            VmwareMachinesMessage::Probe(delivery) => self.probe.receive(delivery, clock),
            VmwareMachinesMessage::Listings(delivery) => self.listings.receive(delivery, clock),
            VmwareMachinesMessage::Machines(delivery) => self.detail.receive(delivery, clock),
            VmwareMachinesMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &VmwareMachineTarget, clock: Clock) -> Option<HoverCard> {
        if let Some(detail) = self.detail.get() {
            let machine = detail
                .machines
                .iter()
                .find(|machine| machine.pid == target.pid)?;
            return self
                .detail
                .hover_card_with(VmwareMachineRow { machine }.hover_card(clock));
        }
        let listing = self
            .listings
            .get()?
            .listings
            .iter()
            .find(|listing| listing.pid == target.pid)?;
        self.listings
            .hover_card_with(VmwareMachineListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let rows = self.machine_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::VmwareMachines,
            &self.expansion,
            identity,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::Desktop(Weight::Regular).into(),
            reading: self,
        })
    }

    fn machine_rows(&self) -> Vec<GroupItem<'_, WidgetGroupExpansionMessage>> {
        if let Some(detail) = self.detail.get() {
            return detail
                .machines
                .iter()
                .map(|machine| {
                    GroupItem::reading(
                        VmwareMachineRow { machine },
                        VmwareMachineTarget::of_pid(machine.pid),
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
                            VmwareMachineListingRow { listing },
                            VmwareMachineTarget::of_pid(listing.pid),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl GroupItemReading for VmwareMachinesGroup {
    fn name(&self) -> &'static str {
        "VMware machines"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "VMware machines".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
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
