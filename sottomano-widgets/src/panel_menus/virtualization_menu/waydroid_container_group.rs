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
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

mod waydroid_status_indicators;

use self::waydroid_status_indicators::{
    ShownWaydroid, WaydroidContainerIndicator, WaydroidSessionIndicator,
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
        let demand = demand.expanded_when(self.expansion.state);
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

    fn stamped_card(&self, shown: ShownWaydroid<'_>, body: Vec<Section>) -> Vec<Section> {
        match shown {
            ShownWaydroid::Detailed(_) => self.detail.hover_card_with(body),
            ShownWaydroid::Listed(_) => self.listing.hover_card_with(body),
        }
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::WaydroidContainer,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::WaydroidContainer.title(),
                IconGlyph::SimpleIcon(SimpleIcon::Android),
                self.shown_state(),
            ),
            &self.expansion,
            WaydroidContainerInteraction::Expansion,
            self.indicator_listing(),
        )
    }

    fn container_indicator<'view>(
        &'view self,
        shown: ShownWaydroid<'view>,
    ) -> GroupIndicator<'view> {
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

    fn indicator_listing(&self) -> IndicatorListing<'_> {
        if let Some(readout) = self.detail.get() {
            return IndicatorListing::Listed(
                self.waydroid_indicators(ShownWaydroid::Detailed(readout)),
            );
        }
        self.listing.indicator_listing(
            |listing| self.waydroid_indicators(ShownWaydroid::Listed(listing)),
            IconGlyph::SimpleIcon(SimpleIcon::Android),
        )
    }

    fn waydroid_indicators<'view>(
        &'view self,
        shown: ShownWaydroid<'view>,
    ) -> Vec<GroupIndicator<'view>> {
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
            .is_some_and(|probe| probe.container_availability == Availability::Available)
        {
            IndicatorState::On
        } else {
            IndicatorState::Off
        }
    }
}
