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
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        process_usage_cells::approximate_memory_text,
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

struct WaydroidContainerIndicator<'a> {
    shown: ShownWaydroid<'a>,
}

impl<'a> WaydroidContainerIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
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
        GroupIndicator::glyph_only(
            "Container",
            Phosphor::Package(Weight::Regular).into(),
            self.shown.container().into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
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

struct WaydroidSessionIndicator<'a> {
    state: &'a WaydroidState,
}

impl<'a> WaydroidSessionIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        let label = match self.state {
            WaydroidState::Session { session_state, .. } => session_state.to_lowercase(),
            WaydroidState::ContainerOnly | WaydroidState::Absent => "no session".to_owned(),
        };
        let state = match self.state {
            WaydroidState::Session { .. } => IndicatorState::On,
            WaydroidState::ContainerOnly | WaydroidState::Absent => IndicatorState::Off,
        };
        GroupIndicator::glyph_only(
            "Session",
            Phosphor::DeviceMobile(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
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
    expansion: IndicatorGroupExpansion,
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
            expansion: IndicatorGroupExpansion::default(),
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
    Expansion(IndicatorGroupExpansionMessage),
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
                .while_headline_shown(snapshot_subscription::<WaydroidListing>(buses.clone()))
                .map(WaydroidContainerMessage::Listing),
            demand
                .while_indicators_shown(snapshot_subscription::<WaydroidReadout>(buses.clone()))
                .map(WaydroidContainerMessage::Readout),
            demand
                .while_indicators_shown(self.container_switch.subscription(buses))
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

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let rows = self.waydroid_indicators();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::WaydroidContainer,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::WaydroidContainer.title(),
                IconGlyph::SimpleIcon(SimpleIcon::Android),
                self.shown_state(),
            ),
            &self.expansion,
            WaydroidContainerInteraction::Expansion,
            rows,
        )
    }

    fn container_indicator<'a>(&'a self, shown: ShownWaydroid<'a>) -> GroupIndicator<'a> {
        let row = WaydroidContainerIndicator { shown }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.stamped_card(shown, WaydroidContainerIndicator { shown }.hover_card())
            }));
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

    fn waydroid_indicators(&self) -> Vec<GroupIndicator<'_>> {
        let Some(shown) = self.shown_waydroid() else {
            return Vec::new();
        };
        let container_indicator = self.container_indicator(shown);
        if shown.container() == ContainerRunState::Stopped {
            return vec![container_indicator];
        }
        let session_indicator = WaydroidSessionIndicator {
            state: shown.state(),
        }
        .indicator()
        .with_hover_card(Box::new(move |_| {
            self.stamped_card(
                shown,
                WaydroidSessionIndicator {
                    state: shown.state(),
                }
                .hover_card(),
            )
        }));
        let session_indicator = match shown.state() {
            WaydroidState::Session { .. } => session_indicator.on_switch_press(
                SwitchPosition::On,
                STOP_SESSION_HINT,
                WaydroidContainerInteraction::StopSession,
            ),
            WaydroidState::ContainerOnly | WaydroidState::Absent => session_indicator
                .on_switch_press(
                    SwitchPosition::Off,
                    START_SESSION_HINT,
                    WaydroidContainerInteraction::StartSession,
                ),
        };
        vec![container_indicator, session_indicator]
    }
}

impl WaydroidContainerGroup {
    fn shown_state(&self) -> IndicatorState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.0 == Availability::Available)
        {
            IndicatorState::On
        } else {
            IndicatorState::Off
        }
    }
}
