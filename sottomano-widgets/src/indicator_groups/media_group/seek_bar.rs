use std::time::{Duration, Instant};

use corredo::value_controls::{
    PlaybackProgress, PlaybackTimeline, SeekAcceptance, SeekBar as PlaybackSeekBar, SeekMessage,
};
use iced::{Element, Task, widget::Row};
use sottomano_desktop_sinks::media_playback_audio::mpris_player_commands;
use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, Media, SeekMode, micros,
};
use sottomano_toml_config::Config;

use super::{
    MediaMessage, PlayerInteraction, PlayerSession, lifted_to_player, player_fallback_message,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
    hover_cards::{MouseGesture, PressAction},
};

#[derive(Debug, Default)]
pub struct SeekBar {
    playback: PlaybackSeekBar<SeekMode>,
}

fn playback_progress(media: &Media, now: Instant) -> PlaybackProgress {
    let Some(played) = media.progress.played(media.status, now) else {
        return PlaybackProgress::Untimed;
    };
    let Some(length) = media.progress.length() else {
        return PlaybackProgress::Untimed;
    };
    PlaybackProgress::Timed { played, length }
}

fn playback_timeline(media: &Media, now: Instant) -> PlaybackTimeline<SeekMode> {
    let progress = playback_progress(media, now);
    let seek_acceptance = match (progress, &media.seek_mode) {
        (PlaybackProgress::Timed { .. }, _) | (_, SeekMode::Absolute(_)) => {
            SeekAcceptance::Accepted
        }
        (PlaybackProgress::Untimed, SeekMode::Relative) => SeekAcceptance::Refused,
    };
    PlaybackTimeline {
        progress,
        session: media.seek_mode.clone(),
        seek_acceptance,
    }
}

impl SeekBar {
    pub fn update(&mut self, message: SeekMessage, media: &Media, now: Instant) -> Option<Command> {
        let target = self
            .playback
            .update(message, &playback_timeline(media, now))?;
        seek_to(media, target, now)
    }

    pub fn player_reported(&mut self, media: &Media, now: Instant) {
        self.playback
            .player_reported(&playback_timeline(media, now));
    }

    pub fn seek_availability(media: &Media, now: Instant) -> SeekAvailability {
        if media.capabilities.seek
            && media.progress.length().is_some()
            && media.progress.played(media.status, now).is_some()
        {
            SeekAvailability::Seekable
        } else {
            SeekAvailability::Fixed
        }
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        media: &Media,
        now: Instant,
    ) -> Option<Element<'a, SeekMessage>> {
        if !media.capabilities.seek {
            return None;
        }
        self.playback.view(
            config.theme.font.icon_size_px,
            &playback_timeline(media, now),
        )
    }
}

fn seek_to(media: &Media, target: Duration, now: Instant) -> Option<Command> {
    match &media.seek_mode {
        SeekMode::Absolute(track) => Some(Command::SetPosition(track.clone(), target)),
        SeekMode::Relative => {
            let played = media.progress.played(media.status, now)?;
            Some(Command::Seek(micros(target) - micros(played)))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekAvailability {
    Seekable,
    Fixed,
}

pub(super) fn seek_position_indicator<'a>(
    config: &'a Config,
    clock: Clock,
    media: &'a Media,
    session: &'a PlayerSession,
) -> GroupIndicator<'a> {
    let bus = media.bus.as_str();
    GroupIndicator::composed(
        "Position",
        IndicatorIcon::IconGlyph {
            glyph: Phosphor::HourglassMedium(Weight::Regular).into(),
            state: IndicatorState::Idle,
        },
        IndicatorValue::drawn(move |_| {
            let bar = session
                .seek_bar
                .view(config, media, clock.instant)
                .map_or_else(|| Row::new().into(), |bar| bar.map(PlayerInteraction::Seek));
            lifted_to_player(bus, bar)
        }),
        Box::new(|_| Vec::new()),
    )
    .with_button_actions([
        PressAction::new(MouseGesture::LeftClick, "jump to this point"),
        PressAction::new(MouseGesture::Scroll, "seek"),
    ])
}

pub(super) fn seek_by(
    session: &mut PlayerSession,
    media: &Media,
    message: SeekMessage,
    now: Instant,
) -> Task<MediaMessage> {
    let Some(command) = session.seek_bar.update(message, media, now) else {
        return Task::none();
    };
    let bus = media.bus.clone();
    Task::future(mpris_player_commands::control(bus.clone(), command)).then(move |dispatched| {
        match dispatched {
            Ok(()) => Task::none(),
            Err(bus_failure) => {
                tracing::warn!("mpris {bus}: seeking: {bus_failure}");
                Task::done(player_fallback_message(
                    &bus,
                    PlayerInteraction::Seek(SeekMessage::Rejected),
                ))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::{
        media_playback_audio::mpris_players::{Capabilities, PlayerSetting, Progress, Status},
        system_protocol_clients::dbus_desktop_buses::OwnerProcess,
    };
    use zbus::zvariant::{ObjectPath, OwnedObjectPath};

    use super::*;
    use crate::clock::Clock;

    fn track(path: &str) -> SeekMode {
        SeekMode::Absolute(OwnedObjectPath::from(ObjectPath::try_from(path).unwrap()))
    }

    fn paused_at(position: Duration, seek_mode: SeekMode, now: Instant) -> Media {
        Media {
            bus: "org.mpris.MediaPlayer2.test".to_owned(),
            owner: OwnerProcess::Unidentified,
            identity: "test".to_owned(),
            desktop_entry: None,
            title: Some("track".to_owned()),
            artists: Vec::new(),
            album: None,
            url: None,
            status: Status::Paused,
            progress: Progress::Timed {
                position,
                sampled_at: now,
                length: Duration::from_secs(300),
                rate: 1.0,
            },
            art_url: None,
            capabilities: Capabilities {
                seek: true,
                ..Capabilities::default()
            },
            queue_order: PlayerSetting::Uncontrollable,
            loop_mode: PlayerSetting::Uncontrollable,
            seek_mode,
        }
    }

    #[test]
    fn a_released_drag_sets_the_position_of_the_track() {
        let now = Clock::real().instant;
        let target = Duration::from_secs(120);
        let media = paused_at(Duration::from_secs(10), track("/t/1"), now);
        let mut bar = SeekBar::default();
        bar.update(SeekMessage::Drag(target.as_secs_f32()), &media, now);
        let command = bar.update(SeekMessage::Release, &media, now);
        assert!(matches!(command, Some(Command::SetPosition(_, position)) if position == target));
    }
}
