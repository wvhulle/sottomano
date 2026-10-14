use std::{thread, time::Duration};

use iced::Task;

use crate::{
    config::Config,
    data::source::{
        logind::permissions::{self, Permissions},
        session_action::SessionAction,
    },
    desktop_shell::hover_tooltips::{HoverTarget, pressable_card},
    message_subscriptions::blocking,
    state::idle::Timeouts,
    widgets::{
        bar_widget::Glyph,
        group_item::GroupItem,
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

    pub fn group_content<'a>(&self, config: &'a Config) -> GroupContent<'a, SessionMessage> {
        let glyph = Glyph::from(Phosphor::Power(Weight::Regular));
        GroupContent::single((!self.permissions.allowed.is_empty()).then(|| {
            GroupItem::element(move |reveal| {
                pressable_card(
                    glyph.view(
                        config.theme.font.icon_size_px,
                        config.theme.palette.bar_foreground.scale_alpha(reveal),
                    ),
                    SessionTarget::Menu.into(),
                    GroupEvent::Tooltip,
                )
            })
        }))
    }
}
