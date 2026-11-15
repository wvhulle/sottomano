use chrono::Locale;

use crate::group_items::{reading_display_state::ReadingDisplayState, reading_row::ReadingRow};
mod account_face;
mod compositor_face;
mod desktop_services_face;
mod kernel_face;
mod login_face;
mod seat_face;
mod service_manager_face;

use std::convert::identity;

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
    group_items::{group_item::GroupItem, reading_row::reading_hover_card},
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        login_session_group::{
            account_face::AccountFace, compositor_face::CompositorFace,
            desktop_services_face::DesktopServicesFace, kernel_face::KernelFace,
            login_face::LoginFace, seat_face::SeatFace, service_manager_face::ServiceManagerFace,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
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
pub enum LoginSessionTarget {
    Kernel,
    Seat,
    Login,
    Account,
    ServiceManager,
    Compositor,
    DesktopServices,
}

impl From<LoginSessionTarget> for HoverTarget {
    fn from(target: LoginSessionTarget) -> Self {
        HoverTarget::LoginSession(target)
    }
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
    Expansion(WidgetGroupExpansionMessage),
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

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: Option<&DesktopBuses>,
        clock: Clock,
    ) -> Task<LoginSessionMessage> {
        self.session.stop();
        self.service_manager.stop();
        self.graphical.stop();
        self.graphical_target.stop();
        let Some(HoverTarget::LoginSession(target)) = hovered else {
            return Task::none();
        };
        if let LoginSessionTarget::Compositor = target {
            return self.graphical.start_hover_fetch(
                (),
                LoginSessionMessage::GraphicalFetched,
                config,
                clock,
            );
        }
        let Some(buses) = buses else {
            return Task::none();
        };
        match target {
            LoginSessionTarget::Login => self.session.start_hover_fetch(
                buses.clone(),
                |delivery| LoginSessionMessage::SessionFetched(Box::new(delivery)),
                config,
                clock,
            ),
            LoginSessionTarget::ServiceManager => self.service_manager.start_hover_fetch(
                buses.clone(),
                LoginSessionMessage::ServiceManagerFetched,
                config,
                clock,
            ),
            LoginSessionTarget::DesktopServices => self.graphical_target.start_hover_fetch(
                buses.clone(),
                LoginSessionMessage::GraphicalTargetFetched,
                config,
                clock,
            ),
            LoginSessionTarget::Kernel
            | LoginSessionTarget::Seat
            | LoginSessionTarget::Account
            | LoginSessionTarget::Compositor => Task::none(),
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
            LoginSessionMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let items = self.identity.get().into_iter().flat_map(|identity| {
            [
                GroupItem::reading(
                    ReadingRow::evaluated(
                        SeatFace {
                            seat: &identity.seat,
                            virtual_terminal: identity.virtual_terminal,
                        },
                        clock,
                        locale,
                    ),
                    LoginSessionTarget::Seat,
                ),
                GroupItem::reading(
                    ReadingRow::evaluated(LoginFace { identity }, clock, locale),
                    LoginSessionTarget::Login,
                ),
                GroupItem::reading(
                    ReadingRow::evaluated(AccountFace { identity }, clock, locale),
                    LoginSessionTarget::Account,
                ),
                GroupItem::reading(
                    ReadingRow::evaluated(ServiceManagerFace, clock, locale),
                    LoginSessionTarget::ServiceManager,
                ),
                GroupItem::reading(
                    ReadingRow::evaluated(CompositorFace { identity }, clock, locale),
                    LoginSessionTarget::Compositor,
                ),
                GroupItem::reading(
                    ReadingRow::evaluated(DesktopServicesFace, clock, locale),
                    LoginSessionTarget::DesktopServices,
                ),
            ]
        });
        let kernel = self.system.get().map(|system| {
            GroupItem::reading(
                ReadingRow::evaluated(KernelFace { system }, clock, locale),
                LoginSessionTarget::Kernel,
            )
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::LoginSession,
            &self.expansion,
            identity,
            kernel.into_iter().chain(items),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::StackSimple(Weight::Regular).into(),
            state: ReadingDisplayState::Idle,
        })
    }

    pub fn refresh(target: LoginSessionTarget, buses: &DesktopBuses) -> Task<LoginSessionMessage> {
        match target {
            LoginSessionTarget::Kernel => first_delivery::<OperatingSystemIdentity>(buses.clone())
                .map(LoginSessionMessage::SystemFetched),
            LoginSessionTarget::Seat
            | LoginSessionTarget::Login
            | LoginSessionTarget::Account
            | LoginSessionTarget::ServiceManager
            | LoginSessionTarget::Compositor
            | LoginSessionTarget::DesktopServices => {
                first_delivery::<SessionIdentity>(buses.clone())
                    .map(LoginSessionMessage::IdentityFetched)
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: LoginSessionTarget, clock: Clock) -> Vec<Section> {
        let session_face =
            |face_sections: Vec<Section>| self.identity.hover_card_with(face_sections);
        let identity = self.identity.get();
        match target {
            LoginSessionTarget::Kernel => {
                let Some(system) = self.system.get() else {
                    return Vec::new();
                };
                self.system
                    .hover_card_with(reading_hover_card(KernelFace { system }, clock))
            }
            LoginSessionTarget::Seat => {
                let Some(identity) = identity else {
                    return Vec::new();
                };
                session_face(reading_hover_card(
                    SeatFace {
                        seat: &identity.seat,
                        virtual_terminal: identity.virtual_terminal,
                    },
                    clock,
                ))
            }
            LoginSessionTarget::Login => {
                let Some(identity) = identity else {
                    return Vec::new();
                };
                let mut sections = session_face(reading_hover_card(LoginFace { identity }, clock));
                sections.extend(self.session.sections(clock));
                sections
            }
            LoginSessionTarget::Account => {
                let Some(identity) = identity else {
                    return Vec::new();
                };
                session_face(reading_hover_card(AccountFace { identity }, clock))
            }
            LoginSessionTarget::Compositor => {
                let Some(identity) = identity else {
                    return Vec::new();
                };
                let mut sections =
                    session_face(reading_hover_card(CompositorFace { identity }, clock));
                sections.extend(self.graphical.sections(clock));
                sections
            }
            LoginSessionTarget::ServiceManager => {
                let mut sections = session_face(reading_hover_card(ServiceManagerFace, clock));
                sections.extend(self.service_manager.sections(clock));
                sections
            }
            LoginSessionTarget::DesktopServices => {
                let mut sections = session_face(reading_hover_card(DesktopServicesFace, clock));
                sections.extend(self.graphical_target.sections(clock));
                sections
            }
        }
    }
}
