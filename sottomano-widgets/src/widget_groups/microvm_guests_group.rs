use std::{borrow::Cow, path::PathBuf};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::microvm_guests::press_guest_power_button;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    microvm_guests::{
        MicrovmGuest, MicrovmGuestListing, MicrovmGuestListings, MicrovmGuests, MicrovmProbe,
        api_socket_of, hypervisor_command_lines::MicrovmHypervisor,
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

const POWER_BUTTON_HINT: &str = "press the guest's power button";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrovmGuestTarget {
    pid: i32,
}

impl MicrovmGuestTarget {
    #[must_use]
    pub const fn of_pid(pid: i32) -> Self {
        Self { pid }
    }
}

impl From<MicrovmGuestTarget> for HoverTarget {
    fn from(target: MicrovmGuestTarget) -> Self {
        HoverTarget::MicrovmGuest(target)
    }
}

fn running_state_text(running: bool) -> &'static str {
    if running { "running" } else { "stopped" }
}

fn running_display_state(running: bool) -> ReadingDisplayState {
    if running {
        ReadingDisplayState::On
    } else {
        ReadingDisplayState::Off
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
        if let Ok(uptime) = clock.wall.duration_since(machine.started_at) {
            detail_entries.push(LabeledValue::new("Uptime", ValueCell::duration(uptime)));
        }
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
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum MicrovmGuestsMessage {
    Probe(SnapshotDelivery<MicrovmProbe>),
    Listings(SnapshotDelivery<MicrovmGuestListings>),
    Machines(SnapshotDelivery<MicrovmGuests>),
    PowerButtonRefused(String),
    Interaction(MicrovmGuestsInteraction),
}

#[derive(Debug, Clone)]
pub enum MicrovmGuestsInteraction {
    PressPowerButton(MicrovmHypervisor, PathBuf),
    Expansion(WidgetGroupExpansionMessage),
}

impl MicrovmGuestsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<MicrovmGuestsMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
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

    #[must_use]
    pub fn hover_card_for(&self, target: &MicrovmGuestTarget, clock: Clock) -> Option<HoverCard> {
        if let Some(detail) = self.detail.get() {
            let machine = detail
                .machines
                .iter()
                .find(|machine| machine.pid == target.pid)?;
            return self
                .detail
                .hover_card_with(MicrovmGuestRow { machine }.hover_card(clock));
        }
        let listing = self
            .listings
            .get()?
            .listings
            .iter()
            .find(|listing| listing.pid == target.pid)?;
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
        if let Some(detail) = self.detail.get() {
            return detail
                .machines
                .iter()
                .map(|machine| {
                    let row = GroupItem::reading(
                        MicrovmGuestRow { machine },
                        MicrovmGuestTarget::of_pid(machine.pid),
                    );
                    match api_socket_of(machine) {
                        Some(api_socket) => row.on_right_press(
                            POWER_BUTTON_HINT,
                            MicrovmGuestsInteraction::PressPowerButton(
                                machine.hypervisor,
                                api_socket.clone(),
                            ),
                        ),
                        None => row,
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
