use std::{iter::once, thread, time::Duration};

use iced::{Element, Task, border::Radius};
use sottomano_desktop_sinks::login_session_presence::logind_session_commands::perform_session_action;
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    login_session_presence::{
        logind_action_permissions::{self, Permissions},
        session_power_actions::SessionAction,
    },
};
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        fixed_panel_group::FixedPanelGroup,
        idle_pause_groups::{ScheduledAfterIdle, Timeouts},
    },
    widgets::{
        configured_theme::text_color,
        context_menu::{
            ContextMenuLine, ContextMenuRow, ContextMenuRowPress, ContextMenuTrailing, context_menu,
        },
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
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
    fn menu_row<'a>(self, config: &Config) -> ContextMenuLine<'a, SessionMessage> {
        let note = match self.idle {
            ScheduledAfterIdle::After(after) => ContextMenuTrailing::Note(format!(
                "after {} idle",
                duration_text(after, config.calendar.locale)
            )),
            ScheduledAfterIdle::Disabled => ContextMenuTrailing::Note("idle: disabled".to_owned()),
            ScheduledAfterIdle::Never if self.needs_authorisation => {
                ContextMenuTrailing::Note("asks for authorisation".to_owned())
            }
            ScheduledAfterIdle::Never => ContextMenuTrailing::Nothing,
        };
        ContextMenuLine::Row(ContextMenuRow {
            leading: Some(
                IconGlyph::from(session_action_icon(self.action))
                    .view(config.theme.font.icon_size_px, text_color),
            ),
            label: Some(self.action.verb().to_owned()),
            trailing: note,
            depth: 0,
            press: ContextMenuRowPress::Enabled(SessionMessage::Press(self.action)),
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
pub struct SessionPowerGroup {
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

impl SessionPowerGroup {
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
                    perform_session_action(action);
                }))
                .discard(),
            ),
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: SessionTarget) -> HoverCard {
        match target {
            SessionTarget::Menu => {
                HoverCard::sourced(HoverCardBody::sections([]), DeliveryTiming::LivePush)
            }
        }
    }

    #[must_use]
    pub fn menu_panel<'a>(
        &self,
        config: &'a Config,
        popup_corners: Radius,
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
                        .then_some(ContextMenuLine::Divider)
                        .into_iter()
                        .chain(once(entry.menu_row(config))),
                )
            })
            .flatten()
            .collect::<Vec<_>>();
        context_menu(config, popup_corners, lines)
    }

    #[must_use]
    pub fn group_content<'a>(&self) -> FixedPanelGroup<'a, SessionMessage> {
        FixedPanelGroup::new((!self.permissions.allowed.is_empty()).then(|| {
            GroupItem::composed(
                ComposedGroupItemFace {
                    name: "Session".into(),
                    icon: GroupItemIcon::IconGlyph {
                        glyph: Phosphor::Power(Weight::Regular).into(),
                        state: ReadingDisplayState::Idle,
                    },
                },
                GroupItemHoverArea::WholeItem(SessionTarget::Menu.into()),
                GroupItemValue::Nothing,
            )
        }))
    }
}
