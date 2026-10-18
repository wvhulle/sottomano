mod account_face;
mod compositor_face;
mod desktop_services_face;
mod kernel_face;
mod login_face;
mod power_policy_face;
mod seat_face;
mod service_manager_face;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    bus::DesktopBuses,
    hostname::OperatingSystemIdentity,
    logind::session_facts::{PowerPolicy, SessionIdentity, SessionInfo},
    systemd_manager::{
        graphical_session::{GraphicalSession, GraphicalSessionTarget},
        user_service_manager::UserServiceManager,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::login_session_group::{
        account_face::AccountFace, compositor_face::CompositorFace,
        desktop_services_face::DesktopServicesFace, kernel_face::KernelFace, login_face::LoginFace,
        power_policy_face::PowerPolicyFace, seat_face::SeatFace,
        service_manager_face::ServiceManagerFace,
    },
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
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
    PowerPolicy,
}

impl From<LoginSessionTarget> for HoverTarget {
    fn from(target: LoginSessionTarget) -> Self {
        HoverTarget::LoginSession(target)
    }
}

#[derive(Default)]
pub struct LoginSessionGroup {
    system: ContinuousReading<OperatingSystemIdentity>,
    identity: ContinuousReading<SessionIdentity>,
    session: OnHoverReading<SessionInfo>,
    service_manager: OnHoverReading<UserServiceManager>,
    graphical: OnHoverReading<GraphicalSession>,
    graphical_target: OnHoverReading<GraphicalSessionTarget>,
    power: OnHoverReading<PowerPolicy>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum LoginSessionMessage {
    SystemFetched(SourceDelivery<OperatingSystemIdentity>),
    IdentityFetched(SourceDelivery<SessionIdentity>),
    SessionFetched(Box<SourceDelivery<SessionInfo>>),
    ServiceManagerFetched(SourceDelivery<UserServiceManager>),
    GraphicalFetched(SourceDelivery<GraphicalSession>),
    GraphicalTargetFetched(SourceDelivery<GraphicalSessionTarget>),
    PowerFetched(Box<SourceDelivery<PowerPolicy>>),
    Expansion(ExpansionMessage),
}

impl LoginSessionGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<LoginSessionMessage> {
        Subscription::batch([
            source_subscription::<OperatingSystemIdentity>(buses.clone())
                .map(LoginSessionMessage::SystemFetched),
            source_subscription::<SessionIdentity>(buses.clone())
                .map(LoginSessionMessage::IdentityFetched),
        ])
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: &DesktopBuses,
    ) -> Task<LoginSessionMessage> {
        self.session.stop();
        self.service_manager.stop();
        self.graphical.stop();
        self.graphical_target.stop();
        self.power.stop();
        match hovered {
            Some(HoverTarget::LoginSession(LoginSessionTarget::Login)) => {
                self.session.start_hover_fetch(
                    buses.clone(),
                    |delivery| LoginSessionMessage::SessionFetched(Box::new(delivery)),
                    config,
                )
            }
            Some(HoverTarget::LoginSession(LoginSessionTarget::ServiceManager)) => {
                self.service_manager.start_hover_fetch(
                    buses.clone(),
                    LoginSessionMessage::ServiceManagerFetched,
                    config,
                )
            }
            Some(HoverTarget::LoginSession(LoginSessionTarget::Compositor)) => self
                .graphical
                .start_hover_fetch((), LoginSessionMessage::GraphicalFetched, config),
            Some(HoverTarget::LoginSession(LoginSessionTarget::DesktopServices)) => {
                self.graphical_target.start_hover_fetch(
                    buses.clone(),
                    LoginSessionMessage::GraphicalTargetFetched,
                    config,
                )
            }
            Some(HoverTarget::LoginSession(LoginSessionTarget::PowerPolicy)) => {
                self.power.start_hover_fetch(
                    buses.clone(),
                    |delivery| LoginSessionMessage::PowerFetched(Box::new(delivery)),
                    config,
                )
            }
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, message: LoginSessionMessage) -> Task<LoginSessionMessage> {
        match message {
            LoginSessionMessage::SystemFetched(delivery) => {
                self.system.receive_keeping_last_reading(delivery);
            }
            LoginSessionMessage::IdentityFetched(delivery) => {
                self.identity.receive_keeping_last_reading(delivery);
            }
            LoginSessionMessage::SessionFetched(delivery) => {
                self.session.receive_delivery(*delivery);
            }
            LoginSessionMessage::ServiceManagerFetched(delivery) => {
                self.service_manager.receive_delivery(delivery);
            }
            LoginSessionMessage::GraphicalFetched(delivery) => {
                self.graphical.receive_delivery(delivery);
            }
            LoginSessionMessage::GraphicalTargetFetched(delivery) => {
                self.graphical_target.receive_delivery(delivery);
            }
            LoginSessionMessage::PowerFetched(delivery) => self.power.receive_delivery(*delivery),
            LoginSessionMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, LoginSessionMessage> {
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
                GroupItem::reading(PowerPolicyFace, LoginSessionTarget::PowerPolicy),
            ]
        });
        let kernel = self
            .system
            .get()
            .map(|system| GroupItem::reading(KernelFace { system }, LoginSessionTarget::Kernel));
        GroupContent::new(
            &self.expansion,
            LoginSessionMessage::Expansion,
            kernel.into_iter().chain(items),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::StackSimple(Weight::Regular).into(),
            handle: GroupHandle::LoginSession,
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
            | LoginSessionTarget::DesktopServices
            | LoginSessionTarget::PowerPolicy => first_delivery::<SessionIdentity>(buses.clone())
                .map(LoginSessionMessage::IdentityFetched),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: LoginSessionTarget) -> Option<Tooltip> {
        let session_face = |face: &dyn BarWidget| self.identity.tooltip_with(face.tooltip());
        let identity = self.identity.get();
        match target {
            LoginSessionTarget::Kernel => self.system.tooltip_with(
                KernelFace {
                    system: self.system.get()?,
                }
                .tooltip(),
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
            .map(|tooltip| tooltip.with_sections(self.session.sections())),
            LoginSessionTarget::Account => session_face(&AccountFace {
                identity: identity?,
            }),
            LoginSessionTarget::Compositor => session_face(&CompositorFace {
                identity: identity?,
            })
            .map(|tooltip| tooltip.with_sections(self.graphical.sections())),
            LoginSessionTarget::ServiceManager => session_face(&ServiceManagerFace)
                .map(|tooltip| tooltip.with_sections(self.service_manager.sections())),
            LoginSessionTarget::DesktopServices => session_face(&DesktopServicesFace)
                .map(|tooltip| tooltip.with_sections(self.graphical_target.sections())),
            LoginSessionTarget::PowerPolicy => session_face(&PowerPolicyFace)
                .map(|tooltip| tooltip.with_sections(self.power.sections())),
        }
    }
}
