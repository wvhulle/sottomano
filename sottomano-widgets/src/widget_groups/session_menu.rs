use std::{thread, time::Duration};

use iced::Task;
use sottomano_config::Config;
use sottomano_sources::{
    background_work::blocking,
    logind::permissions::{self, Permissions},
    session_action::SessionAction,
};
use sottomano_state::idle::Timeouts;

use crate::{
    hover_tooltips::{HoverTarget, pressable_card},
    widgets::{
        bar_widget::Glyph,
        group_item::GroupItem,
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            Freshness, SessionMenuBody, SessionMenuEntry, Tooltip, TooltipBody, WholeBody,
        },
        widget_group::{GroupContent, GroupEvent},
    },
};

const PRESS_SETTLE_DELAY: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTarget {
    Menu,
}

impl From<SessionAction> for Phosphor {
    fn from(action: SessionAction) -> Self {
        let weight = Weight::Regular;
        match action {
            SessionAction::Lock => Phosphor::Lock(weight),
            SessionAction::LockAllSessions => Phosphor::LockKey(weight),
            SessionAction::ScreenOff => Phosphor::Monitor(weight),
            SessionAction::Sleep => Phosphor::Moon(weight),
            SessionAction::Suspend | SessionAction::SuspendThenHibernate => Phosphor::Bed(weight),
            SessionAction::Hibernate => Phosphor::Snowflake(weight),
            SessionAction::HybridSleep => Phosphor::CloudMoon(weight),
            SessionAction::Reboot => Phosphor::ArrowsClockwise(weight),
            SessionAction::RebootToFirmwareSetup => Phosphor::Cpu(weight),
            SessionAction::RebootToBootLoaderMenu => Phosphor::ListBullets(weight),
            SessionAction::Halt => Phosphor::Prohibit(weight),
            SessionAction::PowerOff => Phosphor::Power(weight),
            SessionAction::CancelScheduledShutdown => Phosphor::XCircle(weight),
            SessionAction::LogOut => Phosphor::SignOut(weight),
            SessionAction::TerminateSession => Phosphor::DoorOpen(weight),
            SessionAction::RestartShell => Phosphor::ArrowClockwise(weight),
        }
    }
}

impl From<SessionTarget> for HoverTarget {
    fn from(target: SessionTarget) -> Self {
        HoverTarget::Session(target)
    }
}

#[derive(Default)]
pub struct SessionMenu {
    permissions: Permissions,
}

#[derive(Debug, Clone)]
pub enum SessionMessage {
    PermissionsLoaded(Permissions),
    Press(SessionAction),
}

pub enum Action {
    Run(Task<SessionMessage>),
    Performed(Task<SessionMessage>),
}

impl SessionMenu {
    pub fn load() -> Task<SessionMessage> {
        Task::perform(
            blocking("logind permissions", permissions::read),
            SessionMessage::PermissionsLoaded,
        )
    }

    pub fn update(&mut self, message: SessionMessage) -> Action {
        match message {
            SessionMessage::PermissionsLoaded(permissions) => {
                self.permissions = permissions;
                Action::Run(Task::none())
            }
            SessionMessage::Press(action) => Action::Performed(
                Task::future(blocking("session action", move || {
                    thread::sleep(PRESS_SETTLE_DELAY);
                    action.perform();
                }))
                .discard(),
            ),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: SessionTarget, timeouts: Timeouts) -> Tooltip {
        match target {
            SessionTarget::Menu => Tooltip::sourced(
                TooltipBody::from(WholeBody::SessionMenu(SessionMenuBody {
                    entries: self
                        .permissions
                        .allowed
                        .iter()
                        .map(|allowed| SessionMenuEntry {
                            action: allowed.action,
                            needs_authorisation: allowed.needs_authorisation,
                            idle: timeouts.scheduled(allowed.action),
                        })
                        .collect(),
                })),
                Freshness::Live,
            ),
        }
    }

    #[must_use]
    pub fn group_content<'a>(&self, config: &'a Config) -> GroupContent<'a, SessionMessage> {
        let glyph = Glyph::from(Phosphor::Power(Weight::Regular));
        GroupContent::single((!self.permissions.allowed.is_empty()).then(|| {
            GroupItem::element(move |reveal| {
                pressable_card(
                    glyph.view(
                        config.theme.font.icon_size_px,
                        PaletteColors::from(&config.theme.palette)
                            .bar_foreground
                            .scale_alpha(reveal),
                    ),
                    SessionTarget::Menu.into(),
                    GroupEvent::Tooltip,
                )
            })
        }))
    }
}
