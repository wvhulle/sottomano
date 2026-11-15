use std::{borrow::Cow, future::Future};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    sink_write_errors::GuestPowerError,
    waydroid_container::{start_waydroid_session, stop_waydroid_session},
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    container_run_state::ContainerRunState,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    waydroid_container::{WaydroidListing, WaydroidProbe, WaydroidReadout, WaydroidState},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, dictionary_table},
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
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const WAYDROID_CONTAINER_UNIT: &str = "waydroid-container.service";
const START_SESSION_HINT: &str = "start the Android session";
const STOP_SESSION_HINT: &str = "stop the Android session";
const CONTAINER_SWITCH_FACE: UnitSwitchFace = UnitSwitchFace {
    name: Cow::Borrowed("Android container"),
    icon: Phosphor::Package(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start the Android container",
        turning_off: "stop the Android container",
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaydroidContainerTarget {
    Container,
    Session,
}

impl From<WaydroidContainerTarget> for HoverTarget {
    fn from(target: WaydroidContainerTarget) -> Self {
        HoverTarget::WaydroidContainer(target)
    }
}

#[derive(Clone, Copy)]
enum ShownWaydroid<'a> {
    Detailed(&'a WaydroidReadout),
    Listed(&'a WaydroidListing),
}

impl<'a> ShownWaydroid<'a> {
    fn state(self) -> &'a WaydroidState {
        match self {
            ShownWaydroid::Detailed(readout) => &readout.state,
            ShownWaydroid::Listed(listing) => &listing.state,
        }
    }

    fn container(self) -> ContainerRunState {
        self.state().container_run_state()
    }
}

struct WaydroidContainerRow<'a> {
    shown: ShownWaydroid<'a>,
}

impl GroupItemReading for WaydroidContainerRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Container".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        if let ShownWaydroid::Detailed(WaydroidReadout {
            memory_in_use: Some(memory_in_use),
            ..
        }) = self.shown
            && self.shown.container() == ContainerRunState::Running
        {
            return Some(approximate_memory_text(*memory_in_use));
        }
        Some(self.shown.container().label().to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.shown.container().into()
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let mut detail_entries = vec![
            LabeledValue::new("State", self.shown.container().label()),
            LabeledValue::new("Unit", WAYDROID_CONTAINER_UNIT),
        ];
        if let ShownWaydroid::Detailed(WaydroidReadout {
            memory_in_use: Some(memory_in_use),
            ..
        }) = self.shown
        {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(*memory_in_use),
            ));
        }
        vec![Section::new("Status", dictionary_table(detail_entries))]
    }
}

struct WaydroidSessionRow<'a> {
    state: &'a WaydroidState,
}

impl GroupItemReading for WaydroidSessionRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Session".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Android)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(match self.state {
            WaydroidState::Session { session_state, .. } => session_state.to_lowercase(),
            WaydroidState::ContainerOnly | WaydroidState::Absent => "no session".to_owned(),
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.state {
            WaydroidState::Session { .. } => ReadingDisplayState::On,
            WaydroidState::ContainerOnly | WaydroidState::Absent => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let WaydroidState::Session {
            session_state,
            user_name,
            lcd_density,
        } = self.state
        else {
            return vec![Section::new(
                "Status",
                dictionary_table([LabeledValue::new("Session", "none")]),
            )];
        };
        let mut detail_entries = vec![LabeledValue::new("Session", session_state.clone())];
        if let Some(user_name) = user_name {
            detail_entries.push(LabeledValue::new("User", user_name.clone()));
        }
        if let Some(lcd_density) = lcd_density {
            detail_entries.push(LabeledValue::new("Screen density", lcd_density.clone()));
        }
        vec![Section::new("Status", dictionary_table(detail_entries))]
    }
}

pub struct WaydroidContainerGroup {
    probe: SubscribedSnapshot<WaydroidProbe>,
    listing: SubscribedSnapshot<WaydroidListing>,
    detail: SubscribedSnapshot<WaydroidReadout>,
    container_switch: UnitSwitch,
    expansion: WidgetGroupExpansion,
}

impl Default for WaydroidContainerGroup {
    fn default() -> Self {
        Self {
            probe: SubscribedSnapshot::default(),
            listing: SubscribedSnapshot::default(),
            detail: SubscribedSnapshot::default(),
            container_switch: UnitSwitch::new(
                ScopedSystemdUnit {
                    scope: SystemdUnitScope::SystemWide,
                    unit_name: WAYDROID_CONTAINER_UNIT.to_owned(),
                },
                CONTAINER_SWITCH_FACE,
            ),
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum WaydroidContainerMessage {
    Probe(SnapshotDelivery<WaydroidProbe>),
    Listing(SnapshotDelivery<WaydroidListing>),
    Readout(SnapshotDelivery<WaydroidReadout>),
    ContainerSwitch(UnitSwitchMessage),
    SessionChangeRefused(GuestPowerError),
    Interaction(WaydroidContainerInteraction),
}

#[derive(Debug, Clone)]
pub enum WaydroidContainerInteraction {
    ContainerSwitchPressed,
    StartSession,
    StopSession,
    Expansion(WidgetGroupExpansionMessage),
}

fn refused_session_change(
    session_change: impl Future<Output = Result<(), GuestPowerError>> + Send + 'static,
) -> Task<WaydroidContainerMessage> {
    Task::future(async move { session_change.await.err() })
        .and_then(|refusal| Task::done(WaydroidContainerMessage::SessionChangeRefused(refusal)))
}

impl WaydroidContainerGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<WaydroidContainerMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<WaydroidProbe>(()).map(WaydroidContainerMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<WaydroidListing>(buses.clone()))
                .map(WaydroidContainerMessage::Listing),
            demand
                .while_rows_shown(snapshot_subscription::<WaydroidReadout>(buses.clone()))
                .map(WaydroidContainerMessage::Readout),
            demand
                .while_rows_shown(self.container_switch.subscription(buses))
                .map(WaydroidContainerMessage::ContainerSwitch),
        ])
    }

    pub fn update(
        &mut self,
        message: WaydroidContainerMessage,
        clock: Clock,
    ) -> Task<WaydroidContainerMessage> {
        match message {
            WaydroidContainerMessage::Probe(delivery) => {
                self.probe.receive(delivery, clock);
                Task::none()
            }
            WaydroidContainerMessage::Listing(delivery) => {
                self.listing.receive(delivery, clock);
                Task::none()
            }
            WaydroidContainerMessage::Readout(delivery) => {
                self.detail.receive(delivery, clock);
                Task::none()
            }
            WaydroidContainerMessage::ContainerSwitch(switch_message) => self
                .container_switch
                .update(switch_message, clock)
                .map(WaydroidContainerMessage::ContainerSwitch),
            WaydroidContainerMessage::SessionChangeRefused(refusal) => {
                tracing::warn!(%refusal, "waydroid session change refused");
                Task::none()
            }
            WaydroidContainerMessage::Interaction(
                WaydroidContainerInteraction::ContainerSwitchPressed,
            ) => self
                .container_switch
                .press()
                .map(WaydroidContainerMessage::ContainerSwitch),
            WaydroidContainerMessage::Interaction(WaydroidContainerInteraction::StartSession) => {
                refused_session_change(start_waydroid_session())
            }
            WaydroidContainerMessage::Interaction(WaydroidContainerInteraction::StopSession) => {
                refused_session_change(stop_waydroid_session())
            }
            WaydroidContainerMessage::Interaction(WaydroidContainerInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn shown_waydroid(&self) -> Option<ShownWaydroid<'_>> {
        if let Some(readout) = self.detail.get() {
            return Some(ShownWaydroid::Detailed(readout));
        }
        self.listing.get().map(ShownWaydroid::Listed)
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &WaydroidContainerTarget, clock: Clock) -> Vec<Section> {
        let Some(shown) = self.shown_waydroid() else {
            return Vec::new();
        };
        let body = match target {
            WaydroidContainerTarget::Container => WaydroidContainerRow { shown }.hover_card(clock),
            WaydroidContainerTarget::Session => WaydroidSessionRow {
                state: shown.state(),
            }
            .hover_card(clock),
        };
        match shown {
            ShownWaydroid::Detailed(_) => self.detail.hover_card_with(body),
            ShownWaydroid::Listed(_) => self.listing.hover_card_with(body),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WaydroidContainerInteraction> {
        let rows = self.waydroid_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::WaydroidContainer,
            &self.expansion,
            WaydroidContainerInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: IconGlyph::SimpleIcon(SimpleIcon::Android),
            reading: self,
        })
    }

    fn container_row<'a>(
        &'a self,
        shown: ShownWaydroid<'a>,
    ) -> GroupItem<'a, WaydroidContainerInteraction> {
        let row = GroupItem::reading(
            WaydroidContainerRow { shown },
            WaydroidContainerTarget::Container,
        );
        match self.container_switch.activity() {
            Some(_) => self
                .container_switch
                .attach_switch_press(row, WaydroidContainerInteraction::ContainerSwitchPressed),
            None => row.inert_switch(match shown.container() {
                ContainerRunState::Running => SwitchPosition::On,
                ContainerRunState::Stopped => SwitchPosition::Off,
            }),
        }
    }

    fn waydroid_rows(&self) -> Vec<GroupItem<'_, WaydroidContainerInteraction>> {
        let Some(shown) = self.shown_waydroid() else {
            return Vec::new();
        };
        let container_row = self.container_row(shown);
        if shown.container() == ContainerRunState::Stopped {
            return vec![container_row];
        }
        let session_row = GroupItem::reading(
            WaydroidSessionRow {
                state: shown.state(),
            },
            WaydroidContainerTarget::Session,
        );
        let session_row = match shown.state() {
            WaydroidState::Session { .. } => session_row.on_switch_press(
                SwitchPosition::On,
                STOP_SESSION_HINT,
                WaydroidContainerInteraction::StopSession,
            ),
            WaydroidState::ContainerOnly | WaydroidState::Absent => session_row.on_switch_press(
                SwitchPosition::Off,
                START_SESSION_HINT,
                WaydroidContainerInteraction::StartSession,
            ),
        };
        vec![container_row, session_row]
    }
}

impl GroupItemReading for WaydroidContainerGroup {
    fn name(&self) -> Cow<'static, str> {
        "Android".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Android)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe
            .get()
            .map(|probe| presence_text(probe.0).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.0 == Availability::Available)
        {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Presence",
            dictionary_table(
                self.probe
                    .get()
                    .map(|probe| {
                        vec![LabeledValue::new(
                            "Present",
                            presence_text(probe.0).to_owned(),
                        )]
                    })
                    .unwrap_or_default(),
            ),
        )]
    }
}

fn presence_text(presence: Availability) -> &'static str {
    match presence {
        Availability::Available => "present",
        Availability::Unavailable => "stopped",
    }
}
