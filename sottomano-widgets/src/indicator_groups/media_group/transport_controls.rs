use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, LoopMode, Media, PlayerSetting, QueueOrder, Status,
};

use super::{PlayerInteraction, player_interaction};
use crate::{
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    widgets::phosphor::{Phosphor, Weight},
};

struct TransportButton {
    name: &'static str,
    glyph: Phosphor,
    state: IndicatorState,
    does: &'static str,
    command: Command,
}

impl TransportButton {
    fn indicator<'a>(self, bus: &str) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(self.name, self.glyph.into(), self.state).on_press(
            self.does,
            player_interaction(bus, PlayerInteraction::Command(self.command)),
        )
    }
}

fn shuffle_button(media: &Media) -> Option<TransportButton> {
    let PlayerSetting::Controllable(queue_order) = media.queue_order else {
        return None;
    };
    let state = IndicatorState::from(queue_order);
    Some(TransportButton {
        name: "Shuffle",
        glyph: Phosphor::Shuffle(state.weight()),
        state,
        does: match queue_order {
            QueueOrder::Shuffled => "play in order",
            QueueOrder::InOrder => "shuffle the queue",
        },
        command: Command::SetQueueOrder(queue_order.flipped()),
    })
}

fn previous_button(media: &Media) -> Option<TransportButton> {
    media.capabilities.go_previous.then_some(TransportButton {
        name: "Previous track",
        glyph: Phosphor::SkipBack(Weight::Fill),
        state: IndicatorState::On,
        does: "go to the previous track",
        command: Command::Previous,
    })
}

fn play_or_pause_button(media: &Media) -> Option<TransportButton> {
    let (offered, glyph, does, command) = match media.status {
        Status::Playing => (
            media.capabilities.pause,
            Phosphor::PauseCircle(Weight::Fill),
            "pause",
            Command::Pause,
        ),
        Status::Paused | Status::Stopped => (
            media.capabilities.play,
            Phosphor::PlayCircle(Weight::Fill),
            "play",
            Command::Play,
        ),
    };
    offered.then_some(TransportButton {
        name: "Play or pause",
        glyph,
        state: IndicatorState::On,
        does,
        command,
    })
}

fn next_button(media: &Media) -> Option<TransportButton> {
    media.capabilities.go_next.then_some(TransportButton {
        name: "Next track",
        glyph: Phosphor::SkipForward(Weight::Fill),
        state: IndicatorState::On,
        does: "skip to the next track",
        command: Command::Next,
    })
}

fn loop_button(media: &Media) -> Option<TransportButton> {
    let PlayerSetting::Controllable(mode) = media.loop_mode else {
        return None;
    };
    let state = IndicatorState::from(mode);
    Some(TransportButton {
        name: "Loop",
        glyph: match mode {
            LoopMode::Off | LoopMode::Playlist => Phosphor::Repeat(state.weight()),
            LoopMode::Track => Phosphor::RepeatOnce(state.weight()),
        },
        state,
        does: match mode.cycled() {
            LoopMode::Off => "stop looping",
            LoopMode::Playlist => "loop the playlist",
            LoopMode::Track => "loop this track",
        },
        command: Command::SetLoopMode(mode.cycled()),
    })
}

pub(super) fn transport_button_indicators(
    media: &Media,
) -> impl Iterator<Item = GroupIndicator<'_>> {
    let bus = media.bus.as_str();
    [
        shuffle_button(media),
        previous_button(media),
        play_or_pause_button(media),
        next_button(media),
        loop_button(media),
    ]
    .into_iter()
    .flatten()
    .map(move |button| button.indicator(bus))
}
