use std::{borrow::Cow, future::Future};

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
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        process_usage_cells::approximate_memory_text,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
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

impl<'a> WaydroidContainerRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let label = match self.shown {
            ShownWaydroid::Detailed(WaydroidReadout {
                memory_in_use: Some(memory_in_use),
                ..
            }) if self.shown.container() == ContainerRunState::Running => {
                approximate_memory_text(*memory_in_use)
            }
            ShownWaydroid::Detailed(_) | ShownWaydroid::Listed(_) => {
                self.shown.container().label().to_owned()
            }
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(label),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Container",
                Phosphor::Package(Weight::Regular).into(),
                self.shown.container().into(),
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    fn hover_card(&self) -> Vec<Section> {
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

impl<'a> WaydroidSessionRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let label = match self.state {
            WaydroidState::Session { session_state, .. } => session_state.to_lowercase(),
            WaydroidState::ContainerOnly | WaydroidState::Absent => "no session".to_owned(),
        };
        let state = match self.state {
            WaydroidState::Session { .. } => ReadingDisplayState::On,
            WaydroidState::ContainerOnly | WaydroidState::Absent => ReadingDisplayState::Off,
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(label),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Session",
                IconGlyph::SimpleIcon(SimpleIcon::Android),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    fn hover_card(&self) -> Vec<Section> {
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
}

#[derive(Debug, Clone, Copy)]
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
                .while_summary_shown(snapshot_subscription::<WaydroidListing>(buses.clone()))
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
        }
    }

    pub fn interact(
        &mut self,
        interaction: WaydroidContainerInteraction,
    ) -> Task<WaydroidContainerMessage> {
        match interaction {
            WaydroidContainerInteraction::ContainerSwitchPressed => self
                .container_switch
                .press()
                .map(WaydroidContainerMessage::ContainerSwitch),
            WaydroidContainerInteraction::StartSession => {
                refused_session_change(start_waydroid_session())
            }
            WaydroidContainerInteraction::StopSession => {
                refused_session_change(stop_waydroid_session())
            }
            WaydroidContainerInteraction::Expansion(expansion_message) => {
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

    fn stamped_card(&self, shown: ShownWaydroid<'_>, body: Vec<Section>) -> Vec<Section> {
        match shown {
            ShownWaydroid::Detailed(_) => self.detail.hover_card_with(body),
            ShownWaydroid::Listed(_) => self.listing.hover_card_with(body),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_> {
        let rows = self.waydroid_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::WaydroidContainer,
            GroupSummary::glyph(
                IconGlyph::SimpleIcon(SimpleIcon::Android),
                self.shown_state(),
            ),
            &self.expansion,
            WaydroidContainerInteraction::Expansion,
            rows,
        )
    }

    fn container_row<'a>(&'a self, shown: ShownWaydroid<'a>) -> GroupItem<'a> {
        let row = GroupItem::reading(WaydroidContainerRow { shown }.reading_row()).with_hover_card(
            Box::new(move |_| {
                self.stamped_card(shown, WaydroidContainerRow { shown }.hover_card())
            }),
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

    fn waydroid_rows(&self) -> Vec<GroupItem<'_>> {
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
            }
            .reading_row(),
        )
        .with_hover_card(Box::new(move |_| {
            self.stamped_card(
                shown,
                WaydroidSessionRow {
                    state: shown.state(),
                }
                .hover_card(),
            )
        }));
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

impl WaydroidContainerGroup {
    fn shown_state(&self) -> ReadingDisplayState {
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
}
