use std::mem;

use iced::Task;
use sottomano_desktop_sinks::desktop_notifications::{
    alert_sound_player,
    alert_sound_policy::{SoundChoice, ToastSlot, sound_choice},
};
use sottomano_desktop_sources::desktop_notifications::notification_data::NotificationData;

use super::{NotificationLayer, ToastLayerHost};
use crate::indicator_groups::notification_group::AlertSound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chime {
    Awaiting(SoundChoice),
    Rung,
}

#[must_use]
pub fn arrival_chime(
    data: &NotificationData,
    toast_slot: ToastSlot,
    alert_sound: AlertSound,
) -> Chime {
    match alert_sound {
        AlertSound::Audible => Chime::Awaiting(sound_choice(data, toast_slot)),
        AlertSound::Silent => Chime::Rung,
    }
}

impl<Routed: ToastLayerHost> NotificationLayer<Routed> {
    pub(super) fn play_chime(&self, chime: Chime) -> Task<Routed> {
        match chime {
            Chime::Awaiting(sound_choice) => {
                Task::future(alert_sound_player::play(sound_choice, &self.sound_theme)).discard()
            }
            Chime::Rung => Task::none(),
        }
    }

    pub(super) fn ring_drawn_chimes(&mut self, drawn_toast_ids: &[u32]) -> Task<Routed> {
        let awaiting_chimes: Vec<Chime> = self
            .live
            .iter_mut()
            .filter(|notification| drawn_toast_ids.contains(&notification.data.id))
            .map(|notification| mem::replace(&mut notification.chime, Chime::Rung))
            .collect();
        Task::batch(
            awaiting_chimes
                .into_iter()
                .map(|awaiting_chime| self.play_chime(awaiting_chime)),
        )
    }
}
