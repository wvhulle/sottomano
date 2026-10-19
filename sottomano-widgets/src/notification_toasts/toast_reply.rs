use std::mem;

use iced::{Task, time::Instant, window::Id};
use iced_exwlshell::reexport::KeyboardInteractivity;
use sottomano_config::Config;
use sottomano_sources::desktop_notifications::notification_data::{Outbound, close_reason};

use super::{NotificationLayer, toast_lifecycle::ExitPace};
use crate::{shell_message::Message, toast_messages::ReplyMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplyFocus {
    Typing {
        id: u32,
        panel: Id,
    },
    #[default]
    Idle,
}

impl NotificationLayer {
    pub(super) fn update_reply(
        &mut self,
        id: u32,
        message: ReplyMessage,
        panel_on: &dyn Fn(&str) -> Option<Id>,
        config: &Config,
    ) -> Task<Message> {
        match message {
            ReplyMessage::PointerEntered => self.claim_keyboard(id, panel_on),
            ReplyMessage::PointerLeft if self.draft_of(id).is_empty() => self.release_keyboard(),
            ReplyMessage::PointerLeft => Task::none(),
            ReplyMessage::Edited(text) => {
                if let Some(notif) = self.live.iter_mut().find(|n| n.data.id == id) {
                    notif.reply_draft = text;
                }
                self.sync_countdowns(Instant::now());
                Task::none()
            }
            ReplyMessage::Submitted => self.submit_reply(id, config),
        }
    }

    pub(super) fn release_orphaned_keyboard(&mut self) -> Task<Message> {
        match self.reply_focus {
            ReplyFocus::Typing { id, .. } if !self.showing().any(|n| n.data.id == id) => {
                self.release_keyboard()
            }
            ReplyFocus::Typing { .. } | ReplyFocus::Idle => Task::none(),
        }
    }

    fn draft_of(&self, id: u32) -> &str {
        self.live
            .iter()
            .find(|n| n.data.id == id)
            .map_or("", |n| n.reply_draft.as_str())
    }

    fn claim_keyboard(&mut self, id: u32, panel_on: &dyn Fn(&str) -> Option<Id>) -> Task<Message> {
        if matches!(self.reply_focus, ReplyFocus::Typing { id: typing, .. } if typing == id) {
            return Task::none();
        }
        let panel = self
            .showing()
            .find(|n| n.data.id == id)
            .and_then(|n| n.place.output.as_deref())
            .and_then(panel_on);
        let Some(panel) = panel else {
            return Task::none();
        };
        let released = self.release_keyboard();
        self.reply_focus = ReplyFocus::Typing { id, panel };
        Task::batch([
            released,
            keyboard_interactivity(panel, KeyboardInteractivity::OnDemand),
        ])
    }

    fn release_keyboard(&mut self) -> Task<Message> {
        match mem::take(&mut self.reply_focus) {
            ReplyFocus::Typing { panel, .. } => {
                keyboard_interactivity(panel, KeyboardInteractivity::None)
            }
            ReplyFocus::Idle => Task::none(),
        }
    }

    fn submit_reply(&mut self, id: u32, config: &Config) -> Task<Message> {
        let text = self
            .live
            .iter_mut()
            .find(|n| n.data.id == id)
            .map(|n| mem::take(&mut n.reply_draft))
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Task::none();
        }
        let withdrawn = self.withdraw_showing([id], ExitPace::Dismissed, config);
        let closed = withdrawn
            .into_iter()
            .filter(|id| self.sent_by_dbus_client(*id))
            .map(|id| Outbound::Closed {
                id,
                reason: close_reason::DISMISSED,
            });
        let outbound = [Outbound::Replied { id, text }]
            .into_iter()
            .chain(closed)
            .collect();
        Task::batch([self.emit_in_order(outbound), self.release_keyboard()])
    }
}

fn keyboard_interactivity(panel: Id, interactivity: KeyboardInteractivity) -> Task<Message> {
    Task::done(Message::KeyboardInteractivityChange {
        id: panel,
        keyboard_interactivity: interactivity,
    })
}
