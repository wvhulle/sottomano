use std::{iter::once, time::Duration};

use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent, NestedMenuAction, NestedMenuState},
    theming::{background_strong_color, text_color},
};
use iced::{
    Element, Task,
    widget::{Space, text},
};
use sottomano_desktop_sinks::login_session_presence::logind_session_commands::perform_session_action;
use sottomano_desktop_sources::login_session_presence::{
    logind_action_permissions::{self, ActionAuthorisation, Permissions},
    session_power_actions::SessionAction,
};
use sottomano_toml_config::Config;
use tokio::time::sleep;

use crate::{
    background_work::logged_failure_task,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    indicator_groups::idle_pause_groups::{ResolvedIdleChain, ScheduledAfterIdle},
    widgets::{localized_durations::duration_text, text::resolved_text_face},
};

const PRESS_SETTLE_DELAY: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMenuPresence {
    Shown,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SessionMenuEntry {
    action: SessionAction,
    authorisation: ActionAuthorisation,
    idle: ScheduledAfterIdle,
}

impl SessionMenuEntry {
    fn menu_entry<'view>(self, config: &Config) -> MenuEntry<'view, SessionAction, SessionMessage> {
        let idle_note = |note: String| -> Element<'view, SessionMessage> {
            resolved_text_face(&config.theme.font)
                .text(note)
                .size(config.theme.font.small_text_size_px)
                .style(|theme| text::Style {
                    color: Some(background_strong_color(theme)),
                })
                .into()
        };
        let value = match self.idle {
            ScheduledAfterIdle::After(after) => idle_note(format!(
                "after {} idle",
                duration_text(after, config.calendar.locale)
            )),
            ScheduledAfterIdle::Disabled => idle_note("idle: disabled".to_owned()),
            ScheduledAfterIdle::Never
                if self.authorisation == ActionAuthorisation::AfterChallenge =>
            {
                idle_note("asks for authorisation".to_owned())
            }
            ScheduledAfterIdle::Never => Space::new().into(),
        };
        MenuItem::new(
            MenuRow::new(
                MenuRowContent::new(self.action.verb())
                    .icon(
                        IconGlyph::from(session_action_icon(self.action))
                            .view(config.theme.font.icon_size_px, text_color),
                    )
                    .value(value),
            )
            .on_press(SessionMessage::Press(self.action)),
        )
        .into()
    }
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

#[derive(Default)]
pub struct SessionPowerGroup {
    permissions: Permissions,
    pub menu_state: NestedMenuState<SessionAction>,
}

#[derive(Debug, Clone)]
pub enum SessionMessage {
    PermissionsLoaded(Permissions),
    Press(SessionAction),
    Menu(NestedMenuAction<SessionAction>),
}

pub enum Action {
    Run(Task<SessionMessage>),
    Performed(Task<SessionMessage>),
}

impl SessionPowerGroup {
    pub fn load() -> Task<SessionMessage> {
        Task::perform(
            logind_action_permissions::read(),
            SessionMessage::PermissionsLoaded,
        )
    }

    pub fn update(&mut self, message: SessionMessage) -> Action {
        match message {
            SessionMessage::PermissionsLoaded(permissions) => {
                self.permissions = permissions;
                Action::Run(Task::none())
            }
            SessionMessage::Menu(action) => {
                self.menu_state.perform(action);
                Action::Run(Task::none())
            }
            SessionMessage::Press(action) => Action::Performed(logged_failure_task(
                format!("session action {action:?}"),
                async move {
                    sleep(PRESS_SETTLE_DELAY).await;
                    perform_session_action(action).await
                },
            )),
        }
    }

    #[must_use]
    pub fn menu_entries<'view>(
        &self,
        config: &'view Config,
        idle_chain: ResolvedIdleChain,
    ) -> Vec<MenuEntry<'view, SessionAction, SessionMessage>> {
        self.permissions
            .allowed
            .iter()
            .map(|allowed| SessionMenuEntry {
                action: allowed.action,
                authorisation: allowed.authorisation,
                idle: idle_chain.scheduled(allowed.action),
            })
            .scan(None, |previous_family, entry| {
                let family = entry.action.family();
                let divider = previous_family.is_some_and(|previous| previous != family);
                *previous_family = Some(family);
                Some(
                    divider
                        .then_some(MenuEntry::Divider)
                        .into_iter()
                        .chain(once(entry.menu_entry(config))),
                )
            })
            .flatten()
            .collect()
    }

    #[must_use]
    pub fn menu_presence(&self) -> PowerMenuPresence {
        if self.permissions.allowed.is_empty() {
            return PowerMenuPresence::Hidden;
        }
        PowerMenuPresence::Shown
    }
}
