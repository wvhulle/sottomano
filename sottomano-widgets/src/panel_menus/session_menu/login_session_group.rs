use crate::group_indicators::{
    indicator_group_content::IndicatorListing, indicator_state::IndicatorState,
};
mod account_indicator;
mod compositor_indicator;
mod desktop_services_indicator;
mod kernel_indicator;
mod login_indicator;
mod seat_indicator;
mod service_manager_indicator;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    host_machine_identity::hostname1_os_identity::OperatingSystemIdentity,
    login_session_presence::logind_session_facts::{SessionIdentity, SessionInfo},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        graphical_session_target::{GraphicalSession, GraphicalSessionTarget},
        user_service_manager::UserServiceManager,
    },
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginSessionSource {
    OperatingSystem,
    SessionIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginSessionBusRequest {
    StartSessionFetch,
    StartServiceManagerFetch,
    StartDesktopServicesFetch,
    Refresh(LoginSessionSource),
}

#[derive(Debug, Clone, Copy)]
pub enum LoginSessionInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    StartCompositorFetch,
    StopHoverFetches,
    OnBuses(LoginSessionBusRequest),
}

#[derive(Default)]
pub struct LoginSessionGroup {
    system: SubscribedSnapshot<OperatingSystemIdentity>,
    identity: SubscribedSnapshot<SessionIdentity>,
    session: HoverFetchedSnapshot<SessionInfo>,
    service_manager: HoverFetchedSnapshot<UserServiceManager>,
    graphical: HoverFetchedSnapshot<GraphicalSession>,
    graphical_target: HoverFetchedSnapshot<GraphicalSessionTarget>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum LoginSessionMessage {
    SystemFetched(SnapshotDelivery<OperatingSystemIdentity>),
    IdentityFetched(SnapshotDelivery<SessionIdentity>),
    SessionFetched(Box<SnapshotDelivery<SessionInfo>>),
    ServiceManagerFetched(SnapshotDelivery<UserServiceManager>),
    GraphicalFetched(SnapshotDelivery<GraphicalSession>),
    GraphicalTargetFetched(SnapshotDelivery<GraphicalSessionTarget>),
}

impl LoginSessionGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<LoginSessionMessage> {
        Subscription::batch([
            snapshot_subscription::<OperatingSystemIdentity>(buses.clone())
                .map(LoginSessionMessage::SystemFetched),
            snapshot_subscription::<SessionIdentity>(buses.clone())
                .map(LoginSessionMessage::IdentityFetched),
        ])
    }

    pub fn on_buses(
        &mut self,
        request: LoginSessionBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<LoginSessionMessage> {
        match request {
            LoginSessionBusRequest::StartSessionFetch => self.session.start_hover_fetch(
                buses.clone(),
                |delivery| LoginSessionMessage::SessionFetched(Box::new(delivery)),
                config,
                clock,
            ),
            LoginSessionBusRequest::StartServiceManagerFetch => {
                self.service_manager.start_hover_fetch(
                    buses.clone(),
                    LoginSessionMessage::ServiceManagerFetched,
                    config,
                    clock,
                )
            }
            LoginSessionBusRequest::StartDesktopServicesFetch => {
                self.graphical_target.start_hover_fetch(
                    buses.clone(),
                    LoginSessionMessage::GraphicalTargetFetched,
                    config,
                    clock,
                )
            }
            LoginSessionBusRequest::Refresh(LoginSessionSource::OperatingSystem) => {
                first_delivery::<OperatingSystemIdentity>(buses.clone())
                    .map(LoginSessionMessage::SystemFetched)
            }
            LoginSessionBusRequest::Refresh(LoginSessionSource::SessionIdentity) => {
                first_delivery::<SessionIdentity>(buses.clone())
                    .map(LoginSessionMessage::IdentityFetched)
            }
        }
    }

    pub fn update(
        &mut self,
        message: LoginSessionMessage,
        clock: Clock,
    ) -> Task<LoginSessionMessage> {
        match message {
            LoginSessionMessage::SystemFetched(delivery) => {
                self.system.receive(delivery, clock);
            }
            LoginSessionMessage::IdentityFetched(delivery) => {
                self.identity.receive(delivery, clock);
            }
            LoginSessionMessage::SessionFetched(delivery) => {
                self.session.receive(*delivery, clock);
            }
            LoginSessionMessage::ServiceManagerFetched(delivery) => {
                self.service_manager.receive(delivery, clock);
            }
            LoginSessionMessage::GraphicalFetched(delivery) => {
                self.graphical.receive(delivery, clock);
            }
            LoginSessionMessage::GraphicalTargetFetched(delivery) => {
                self.graphical_target.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: LoginSessionInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<LoginSessionMessage> {
        match interaction {
            LoginSessionInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            LoginSessionInteraction::StartCompositorFetch => self.graphical.start_hover_fetch(
                (),
                LoginSessionMessage::GraphicalFetched,
                config,
                clock,
            ),
            LoginSessionInteraction::StopHoverFetches => {
                self.session.stop();
                self.service_manager.stop();
                self.graphical.stop();
                self.graphical_target.stop();
                Task::none()
            }
            LoginSessionInteraction::OnBuses(_) => Task::none(),
        }
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::LoginSession,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::LoginSession.title(),
                Phosphor::StackSimple(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            LoginSessionInteraction::Expansion,
            IndicatorListing::Listed(
                self.kernel_rows()
                    .into_iter()
                    .chain(self.identity.get().into_iter().flat_map(|identity| {
                        [
                            self.seat_row(identity),
                            self.login_row(identity),
                            self.account_row(identity),
                            self.service_manager_row(),
                            self.compositor_row(identity),
                            self.desktop_services_row(),
                        ]
                    }))
                    .collect(),
            ),
        )
    }
}

pub(super) fn identity_refresh() -> LoginSessionInteraction {
    LoginSessionInteraction::OnBuses(LoginSessionBusRequest::Refresh(
        LoginSessionSource::SessionIdentity,
    ))
}
