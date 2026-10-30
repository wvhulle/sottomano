use std::{iter::once, thread, time::Duration};

use iced::{Element, Task};
use sottomano_config::Config;
use sottomano_sources::{
    blocking_background_work::blocking,
    login_session_presence::{
        logind_action_permissions::{self, Permissions},
        session_power_actions::SessionAction,
    },
};

use crate::{
    hover_tooltips::{HoverTarget, PressHints, hoverable},
    widget_groups::idle_timeouts_group::{ScheduledAfterIdle, Timeouts},
    widgets::{
        bar_widget::Glyph,
        configured_theme::text_color,
        context_menu::{MenuLine, MenuRow, Trailing, context_menu},
        group_item::GroupItem,
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            TooltipBody,
            tooltip_source_delivery::{DeliveryTiming, Tooltip},
        },
        widget_group::{GroupContent, GroupEvent},
        widget_state::{StateTint, WidgetState},
    },
};

const PRESS_SETTLE_DELAY: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SessionMenuEntry {
    action: SessionAction,
    needs_authorisation: bool,
    idle: ScheduledAfterIdle,
}

impl SessionMenuEntry {
    fn menu_row<'a>(self, config: &Config) -> MenuLine<'a, SessionMessage> {
        let note = match self.idle {
            ScheduledAfterIdle::After(after) => Trailing::Note(format!(
                "after {} idle",
                duration_text(after, config.calendar.locale)
            )),
            ScheduledAfterIdle::Disabled => Trailing::Note("idle: disabled".to_owned()),
            ScheduledAfterIdle::Never if self.needs_authorisation => {
                Trailing::Note("asks for authorisation".to_owned())
            }
            ScheduledAfterIdle::Never => Trailing::Nothing,
        };
        MenuLine::Row(MenuRow {
            leading: Some(
                Glyph::from(session_action_icon(self.action))
                    .view(config.theme.font.icon_size_px, text_color),
            ),
            label: Some(self.action.verb().to_owned()),
            trailing: note,
            enabled: true,
            depth: 0,
            on_press: Some(SessionMessage::Press(self.action)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTarget {
    Menu,
}

#[must_use]
pub fn session_action_icon(action: SessionAction) -> Phosphor {
    {
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
    pub const NAME: &str = "Session";

    pub fn load() -> Task<SessionMessage> {
        Task::perform(
            blocking("logind permissions", logind_action_permissions::read),
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
    pub fn tooltip_for(&self, target: SessionTarget) -> Tooltip {
        match target {
            SessionTarget::Menu => {
                Tooltip::sourced(TooltipBody::sections([]), DeliveryTiming::LivePush)
            }
        }
    }

    #[must_use]
    pub fn menu_panel<'a>(
        &self,
        config: &'a Config,
        timeouts: Timeouts,
    ) -> Element<'a, SessionMessage> {
        let lines = self
            .permissions
            .allowed
            .iter()
            .map(|allowed| SessionMenuEntry {
                action: allowed.action,
                needs_authorisation: allowed.needs_authorisation,
                idle: timeouts.scheduled(allowed.action),
            })
            .scan(None, |previous_family, entry| {
                let family = entry.action.family();
                let divider = previous_family.is_some_and(|previous| previous != family);
                *previous_family = Some(family);
                Some(
                    divider
                        .then_some(MenuLine::Divider)
                        .into_iter()
                        .chain(once(entry.menu_row(config))),
                )
            })
            .flatten()
            .collect::<Vec<_>>();
        context_menu(config, lines)
    }

    #[must_use]
    pub fn group_content<'a>(&self, config: &'a Config) -> GroupContent<'a, SessionMessage> {
        let glyph = Glyph::from(Phosphor::Power(Weight::Regular));
        GroupContent::single(
            Self::NAME,
            (!self.permissions.allowed.is_empty()).then(|| {
                GroupItem::element(move |reveal| {
                    hoverable(
                        glyph.view(config.theme.font.icon_size_px, {
                            let tint = StateTint {
                                state: WidgetState::Idle,
                                opacity: reveal,
                            };
                            move |theme| tint.color(theme)
                        }),
                        SessionTarget::Menu.into(),
                        PressHints::default(),
                        GroupEvent::Tooltip,
                    )
                })
            }),
        )
    }
}
