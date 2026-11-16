use crate::group_items::reading_display_state::ReadingDisplayState;
mod account_face;
mod compositor_face;
mod desktop_services_face;
mod kernel_face;
mod login_face;
mod seat_face;
mod service_manager_face;

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
    group_items::group_item::GroupItem,
    widget_groups::{
        group_summary::GroupSummary,
        login_session_group::{
            account_face::AccountFace,
            compositor_face::{CompositorFace, graphical_session_sections},
            desktop_services_face::{DesktopServicesFace, graphical_session_target_sections},
            kernel_face::KernelFace,
            login_face::{LoginFace, session_info_sections},
            seat_face::SeatFace,
            service_manager_face::{ServiceManagerFace, user_service_manager_sections},
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
    Expansion(WidgetGroupExpansionMessage),
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
    expansion: WidgetGroupExpansion,
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

    fn identity_items<'a>(
        &'a self,
        identity: &'a SessionIdentity,
    ) -> [GroupItem<'a, LoginSessionInteraction>; 6] {
        [
            GroupItem::reading(
                SeatFace {
                    seat: &identity.seat,
                    virtual_terminal: identity.virtual_terminal,
                }
                .reading_row(),
            )
            .with_hover_card(Box::new(move |_| {
                self.identity.hover_card_with(
                    SeatFace {
                        seat: &identity.seat,
                        virtual_terminal: identity.virtual_terminal,
                    }
                    .hover_card(),
                )
            }))
            .refreshed_by(identity_refresh()),
            GroupItem::reading(LoginFace { identity }.reading_row())
                .with_hover_card(Box::new(move |_| {
                    let mut sections = self
                        .identity
                        .hover_card_with(LoginFace { identity }.hover_card());
                    sections.extend(self.session.sections_described_by(session_info_sections));
                    sections
                }))
                .fetching_on_hover(
                    LoginSessionInteraction::OnBuses(LoginSessionBusRequest::StartSessionFetch),
                    LoginSessionInteraction::StopHoverFetches,
                )
                .refreshed_by(identity_refresh()),
            GroupItem::reading(AccountFace { identity }.reading_row())
                .with_hover_card(Box::new(move |_| {
                    self.identity
                        .hover_card_with(AccountFace { identity }.hover_card())
                }))
                .refreshed_by(identity_refresh()),
            GroupItem::reading(ServiceManagerFace::reading_row())
                .with_hover_card(Box::new(move |_| {
                    let mut sections = self
                        .identity
                        .hover_card_with(ServiceManagerFace::hover_card());
                    sections.extend(
                        self.service_manager
                            .sections_described_by(user_service_manager_sections),
                    );
                    sections
                }))
                .fetching_on_hover(
                    LoginSessionInteraction::OnBuses(
                        LoginSessionBusRequest::StartServiceManagerFetch,
                    ),
                    LoginSessionInteraction::StopHoverFetches,
                )
                .refreshed_by(identity_refresh()),
            GroupItem::reading(CompositorFace { identity }.reading_row())
                .with_hover_card(Box::new(move |_| {
                    let mut sections = self
                        .identity
                        .hover_card_with(CompositorFace { identity }.hover_card());
                    sections.extend(
                        self.graphical
                            .sections_described_by(graphical_session_sections),
                    );
                    sections
                }))
                .fetching_on_hover(
                    LoginSessionInteraction::StartCompositorFetch,
                    LoginSessionInteraction::StopHoverFetches,
                )
                .refreshed_by(identity_refresh()),
            GroupItem::reading(DesktopServicesFace::reading_row())
                .with_hover_card(Box::new(move |_| {
                    let mut sections = self
                        .identity
                        .hover_card_with(DesktopServicesFace::hover_card());
                    sections.extend(self.graphical_target.sections_described_by(|snapshot| {
                        graphical_session_target_sections(snapshot)
                    }));
                    sections
                }))
                .fetching_on_hover(
                    LoginSessionInteraction::OnBuses(
                        LoginSessionBusRequest::StartDesktopServicesFetch,
                    ),
                    LoginSessionInteraction::StopHoverFetches,
                )
                .refreshed_by(identity_refresh()),
        ]
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, LoginSessionInteraction> {
        let mut rows = Vec::new();
        if let Some(system) = self.system.get() {
            rows.push(
                GroupItem::reading(KernelFace { system }.reading_row())
                    .with_hover_card(Box::new(move |_| {
                        self.system
                            .hover_card_with(KernelFace { system }.hover_card())
                    }))
                    .refreshed_by(LoginSessionInteraction::OnBuses(
                        LoginSessionBusRequest::Refresh(LoginSessionSource::OperatingSystem),
                    )),
            );
        }
        if let Some(identity) = self.identity.get() {
            rows.extend(self.identity_items(identity));
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::LoginSession,
            GroupSummary::glyph(
                Phosphor::StackSimple(Weight::Regular).into(),
                ReadingDisplayState::Idle,
            ),
            &self.expansion,
            LoginSessionInteraction::Expansion,
            rows,
        )
    }
}

fn identity_refresh() -> LoginSessionInteraction {
    LoginSessionInteraction::OnBuses(LoginSessionBusRequest::Refresh(
        LoginSessionSource::SessionIdentity,
    ))
}
