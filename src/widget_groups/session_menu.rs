use std::{thread, time::Duration};

use iced::{Element, Task};

use crate::{
    config::Config,
    data::source::{
        logind::permissions::{self, Permissions},
        session_action::SessionAction,
    },
    desktop_shell::{
        Message,
        hover_tooltips::{HoverTarget, pressable_card},
        monitor_surfaces::Position,
    },
    message_subscriptions::blocking,
    state::idle::Timeouts,
    widgets::{
        activity_line::activity_line,
        bar_widget::Glyph,
        pager_slot::PagerSlot,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            Freshness, SessionMenuBody, SessionMenuEntry, Tooltip, TooltipBody, WholeBody,
        },
    },
};

const PRESS_SETTLE_DELAY: Duration = Duration::from_millis(600);
const RESTING_INDICATOR_OPACITY: f32 = 0.12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTarget {
    Menu,
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
        blocking("logind permissions", permissions::read).map(SessionMessage::PermissionsLoaded)
    }

    pub fn update(&mut self, message: SessionMessage) -> Action {
        match message {
            SessionMessage::PermissionsLoaded(permissions) => {
                self.permissions = permissions;
                Action::Run(Task::none())
            }
            SessionMessage::Press(action) => Action::Performed(
                blocking("session action", move || {
                    thread::sleep(PRESS_SETTLE_DELAY);
                    action.perform();
                })
                .discard(),
            ),
        }
    }

    pub fn tooltip_for(&self, target: SessionTarget, timeouts: Timeouts) -> Tooltip {
        match target {
            SessionTarget::Menu => Tooltip::sourced(
                TooltipBody::Whole(WholeBody::SessionMenu(SessionMenuBody {
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

    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, Message>> {
        if self.permissions.allowed.is_empty() {
            return None;
        }
        let foreground = config.theme.palette.bar_foreground;
        let handle = pressable_card(
            Glyph::from(Phosphor::Power(Weight::Regular))
                .view(config.theme.font.icon_size_px, foreground),
            SessionTarget::Menu.into(),
            Message::Tooltip,
        );
        let indicator = activity_line(
            foreground,
            RESTING_INDICATOR_OPACITY,
            config.panel_bar.indicator_thickness,
        );
        Some(PagerSlot::new(handle, indicator).view(config, position))
    }
}
