mod account_face;
mod compositor_face;
mod console_face;
mod login_face;
mod power_policy_face;
mod seat_face;
mod user_services_face;

use iced::Task;

use crate::{
    config::Config,
    data::source::{
        Delivery,
        logind::session_facts::{self, PowerPolicy, SessionIdentity, SessionInfo},
        systemd_manager::graphical_session::{GraphicalSession, GraphicalSessionTarget},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::blocking,
    widget_groups::login_session_group::{
        account_face::AccountFace, compositor_face::CompositorFace, console_face::ConsoleFace,
        login_face::LoginFace, power_policy_face::PowerPolicyFace, seat_face::SeatFace,
        user_services_face::UserServicesFace,
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        tooltip_body::{Freshness, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginSessionTarget {
    Seat,
    Console,
    Login,
    Account,
    Compositor,
    UserServices,
    PowerPolicy,
}

impl From<LoginSessionTarget> for HoverTarget {
    fn from(target: LoginSessionTarget) -> Self {
        HoverTarget::LoginSession(target)
    }
}

#[derive(Default)]
pub struct LoginSessionGroup {
    identity: Option<SessionIdentity>,
    session: OnHoverReading<SessionInfo>,
    graphical: OnHoverReading<GraphicalSession>,
    graphical_target: OnHoverReading<GraphicalSessionTarget>,
    power: OnHoverReading<PowerPolicy>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum LoginSessionMessage {
    IdentityLoaded(Option<SessionIdentity>),
    SessionFetched(Box<Delivery<SessionInfo>>),
    GraphicalFetched(Delivery<GraphicalSession>),
    GraphicalTargetFetched(Delivery<GraphicalSessionTarget>),
    PowerFetched(Box<Delivery<PowerPolicy>>),
    Expansion(ExpansionMessage),
}

impl LoginSessionGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn load() -> Task<LoginSessionMessage> {
        blocking("session identity", session_facts::identity)
            .map(LoginSessionMessage::IdentityLoaded)
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<LoginSessionMessage> {
        self.session.stop();
        self.graphical.stop();
        self.graphical_target.stop();
        self.power.stop();
        match hovered {
            Some(HoverTarget::LoginSession(LoginSessionTarget::Login)) => self.session.poll(
                (),
                |delivery| LoginSessionMessage::SessionFetched(Box::new(delivery)),
                config,
            ),
            Some(HoverTarget::LoginSession(LoginSessionTarget::Compositor)) => {
                self.graphical
                    .poll((), LoginSessionMessage::GraphicalFetched, config)
            }
            Some(HoverTarget::LoginSession(LoginSessionTarget::UserServices)) => self
                .graphical_target
                .poll((), LoginSessionMessage::GraphicalTargetFetched, config),
            Some(HoverTarget::LoginSession(LoginSessionTarget::PowerPolicy)) => self.power.poll(
                (),
                |delivery| LoginSessionMessage::PowerFetched(Box::new(delivery)),
                config,
            ),
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, message: LoginSessionMessage) -> Task<LoginSessionMessage> {
        match message {
            LoginSessionMessage::IdentityLoaded(identity) => self.identity = identity,
            LoginSessionMessage::SessionFetched(delivery) => self.session.receive(*delivery),
            LoginSessionMessage::GraphicalFetched(delivery) => self.graphical.receive(delivery),
            LoginSessionMessage::GraphicalTargetFetched(delivery) => {
                self.graphical_target.receive(delivery);
            }
            LoginSessionMessage::PowerFetched(delivery) => self.power.receive(*delivery),
            LoginSessionMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, LoginSessionMessage> {
        let items = self.identity.iter().flat_map(|identity| {
            let seat = identity
                .seat
                .as_deref()
                .map(|seat| GroupItem::reading(SeatFace { seat }, LoginSessionTarget::Seat));
            let console = identity.console.as_deref().map(|console| {
                GroupItem::reading(
                    ConsoleFace {
                        console,
                        virtual_terminal: identity.virtual_terminal,
                    },
                    LoginSessionTarget::Console,
                )
            });
            seat.into_iter().chain(console).chain([
                GroupItem::reading(LoginFace { identity }, LoginSessionTarget::Login),
                GroupItem::reading(AccountFace { identity }, LoginSessionTarget::Account),
                GroupItem::reading(CompositorFace { identity }, LoginSessionTarget::Compositor),
                GroupItem::reading(UserServicesFace, LoginSessionTarget::UserServices),
                GroupItem::reading(PowerPolicyFace, LoginSessionTarget::PowerPolicy),
            ])
        });
        GroupContent::new(&self.expansion, LoginSessionMessage::Expansion, items)
    }

    pub fn tooltip_for(&self, target: LoginSessionTarget) -> Option<Tooltip> {
        let identity = self.identity.as_ref()?;
        let sourced = |face: &dyn BarWidget| Tooltip::sourced(face.tooltip(), Freshness::Live);
        Some(match target {
            LoginSessionTarget::Seat => sourced(&SeatFace {
                seat: identity.seat.as_deref()?,
            }),
            LoginSessionTarget::Console => sourced(&ConsoleFace {
                console: identity.console.as_deref()?,
                virtual_terminal: identity.virtual_terminal,
            }),
            LoginSessionTarget::Login => sourced(&LoginFace { identity })
                .with_sections(self.session.sections(SessionInfo::sections)),
            LoginSessionTarget::Account => sourced(&AccountFace { identity }),
            LoginSessionTarget::Compositor => sourced(&CompositorFace { identity })
                .with_sections(self.graphical.sections(GraphicalSession::sections)),
            LoginSessionTarget::UserServices => sourced(&UserServicesFace).with_sections(
                self.graphical_target
                    .sections(GraphicalSessionTarget::sections),
            ),
            LoginSessionTarget::PowerPolicy => {
                sourced(&PowerPolicyFace).with_sections(self.power.sections(PowerPolicy::sections))
            }
        })
    }
}
