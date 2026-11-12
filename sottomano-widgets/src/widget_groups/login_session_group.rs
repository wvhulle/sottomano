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
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
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
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
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
    system: LatestSnapshot<OperatingSystemIdentity, HidesFetchFailures>,
    identity: LatestSnapshot<SessionIdentity, HidesFetchFailures>,
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
    Interaction(LoginSessionInteraction),
}

#[derive(Debug, Clone)]
pub enum LoginSessionInteraction {
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
            LoginSessionMessage::Interaction(interaction) => self.interact(interaction),
        }
        Task::none()
    }

    fn interact(&mut self, interaction: LoginSessionInteraction) {
        match interaction {
            LoginSessionInteraction::Expansion(expansion) => {
                self.expansion.update(expansion);
            }
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, LoginSessionInteraction> {
        let items = self.identity.get().into_iter().flat_map(|identity| {
            [
                GroupItem::reading(
                    SeatFace {
                        seat: &identity.seat,
                        virtual_terminal: identity.virtual_terminal,
                    },
                    LoginSessionTarget::Seat,
                ),
                GroupItem::reading(LoginFace { identity }, LoginSessionTarget::Login),
                GroupItem::reading(AccountFace { identity }, LoginSessionTarget::Account),
                GroupItem::reading(ServiceManagerFace, LoginSessionTarget::ServiceManager),
                GroupItem::reading(CompositorFace { identity }, LoginSessionTarget::Compositor),
                GroupItem::reading(DesktopServicesFace, LoginSessionTarget::DesktopServices),
            ]
        });
        let kernel = self
            .system
            .get()
            .map(|system| GroupItem::reading(KernelFace { system }, LoginSessionTarget::Kernel));
        WidgetGroupContent::expandable(
            WidgetGroupFamily::LoginSession,
            &self.expansion,
            LoginSessionInteraction::Expansion,
            kernel.into_iter().chain(items),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::StackSimple(Weight::Regular).into(),
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
    pub fn hover_card_for(&self, target: LoginSessionTarget, clock: Clock) -> Option<HoverCard> {
        let session_face =
            |face: &dyn GroupItemReading| self.identity.hover_card_with(face.hover_card(clock));
        let identity = self.identity.get();
        match target {
            LoginSessionTarget::Kernel => self.system.hover_card_with(
                (KernelFace {
                    system: self.system.get()?,
                })
                .hover_card(clock),
            ),
            LoginSessionTarget::Seat => {
                let identity = identity?;
                session_face(&SeatFace {
                    seat: &identity.seat,
                    virtual_terminal: identity.virtual_terminal,
                })
            }
            LoginSessionTarget::Login => session_face(&LoginFace {
                identity: identity?,
            })
            .map(|hover_card| hover_card.with_groups(self.session.sections(clock))),
            LoginSessionTarget::Account => session_face(&AccountFace {
                identity: identity?,
            }),
            LoginSessionTarget::Compositor => session_face(&CompositorFace {
                identity: identity?,
            })
            .map(|hover_card| hover_card.with_groups(self.graphical.sections(clock))),
            LoginSessionTarget::ServiceManager => session_face(&ServiceManagerFace)
                .map(|hover_card| hover_card.with_groups(self.service_manager.sections(clock))),
            LoginSessionTarget::DesktopServices => session_face(&DesktopServicesFace)
                .map(|hover_card| hover_card.with_groups(self.graphical_target.sections(clock))),
        }
    }
}
