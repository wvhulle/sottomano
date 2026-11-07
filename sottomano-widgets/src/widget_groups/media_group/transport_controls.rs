use iced::{Alignment, Element, widget::Row};
use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, LoopMode, Media, QueueOrder, Status,
};
use sottomano_toml_config::Config;

use crate::{
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    hover_cards::{HoverCardMessage, PressHints, hoverable},
    widget_groups::media_group::{MediaTarget, PlayerTarget},
    widgets::{
        glyph_button,
        phosphor::{Phosphor, Weight},
    },
};

pub struct TransportControls<'c, Message> {
    config: &'c Config,
    media: &'c Media,
    reveal: f32,
    on_command: fn(Command) -> Message,
    on_hover_card: fn(HoverCardMessage) -> Message,
}

struct TransportPress {
    target: MediaTarget,
    does: &'static str,
    command: Command,
}

impl<'c, Message: Clone> TransportControls<'c, Message> {
    pub fn new(
        config: &'c Config,
        media: &'c Media,
        reveal: f32,
        on_command: fn(Command) -> Message,
        on_hover_card: fn(HoverCardMessage) -> Message,
    ) -> Self {
        Self {
            config,
            media,
            reveal,
            on_command,
            on_hover_card,
        }
    }

    fn primary_size(&self) -> f32 {
        self.config.theme.font.icon_size_px
    }

    fn foreground(&self) -> ReadingStateTint {
        ReadingStateTint {
            state: ReadingDisplayState::On,
            opacity: self.reveal,
        }
    }

    fn toggle<'a>(
        &self,
        icon: Phosphor,
        state: ReadingDisplayState,
        press: TransportPress,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        self.control(
            icon.with_weight(state.weight()),
            ReadingStateTint {
                state,
                opacity: self.reveal,
            },
            press,
        )
    }

    fn control<'a>(
        &self,
        glyph: Phosphor,
        tint: ReadingStateTint,
        press: TransportPress,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        hoverable(
            glyph_button(
                glyph,
                self.primary_size(),
                move |theme| tint.color(theme),
                (self.on_command)(press.command),
            ),
            PlayerTarget::of(&self.media.bus, press.target).into(),
            PressHints {
                left: Some(press.does),
                ..PressHints::default()
            },
            self.on_hover_card,
        )
    }

    fn shuffle<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        let queue_order = self.media.queue_order?;
        Some(self.toggle(
            Phosphor::Shuffle(Weight::Regular),
            ReadingDisplayState::from(queue_order),
            TransportPress {
                target: MediaTarget::Shuffle,
                does: match queue_order {
                    QueueOrder::Shuffled => "play in order",
                    QueueOrder::InOrder => "shuffle the queue",
                },
                command: Command::SetQueueOrder(queue_order.flipped()),
            },
        ))
    }

    fn previous<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.go_previous.then(|| {
            self.control(
                Phosphor::SkipBack(Weight::Fill),
                self.foreground(),
                TransportPress {
                    target: MediaTarget::Previous,
                    does: "go to the previous track",
                    command: Command::Previous,
                },
            )
        })
    }

    fn play_or_pause<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        match self.media.status {
            Status::Playing => self.pause(),
            Status::Paused | Status::Stopped => self.play(),
        }
    }

    fn play<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.play.then(|| {
            self.control(
                Phosphor::PlayCircle(Weight::Fill),
                self.foreground(),
                TransportPress {
                    target: MediaTarget::PlayPause,
                    does: "play",
                    command: Command::Play,
                },
            )
        })
    }

    fn pause<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.pause.then(|| {
            self.control(
                Phosphor::PauseCircle(Weight::Fill),
                self.foreground(),
                TransportPress {
                    target: MediaTarget::PlayPause,
                    does: "pause",
                    command: Command::Pause,
                },
            )
        })
    }

    fn next<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.go_next.then(|| {
            self.control(
                Phosphor::SkipForward(Weight::Fill),
                self.foreground(),
                TransportPress {
                    target: MediaTarget::Next,
                    does: "skip to the next track",
                    command: Command::Next,
                },
            )
        })
    }

    fn loop_mode<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        let mode = self.media.loop_mode?;
        let glyph = match mode {
            LoopMode::Off | LoopMode::Playlist => Phosphor::Repeat(Weight::Regular),
            LoopMode::Track => Phosphor::RepeatOnce(Weight::Regular),
        };
        Some(self.toggle(
            glyph,
            ReadingDisplayState::from(mode),
            TransportPress {
                target: MediaTarget::Loop,
                does: match mode.cycled() {
                    LoopMode::Off => "stop looping",
                    LoopMode::Playlist => "loop the playlist",
                    LoopMode::Track => "loop this track",
                },
                command: Command::SetLoopMode(mode.cycled()),
            },
        ))
    }
}

impl<'a, Message: 'a + Clone> From<TransportControls<'_, Message>> for Element<'a, Message> {
    fn from(controls: TransportControls<'_, Message>) -> Self {
        let buttons = [
            controls.shuffle(),
            controls.previous(),
            controls.play_or_pause(),
            controls.next(),
            controls.loop_mode(),
        ];
        Row::with_children(buttons.into_iter().flatten())
            .spacing(controls.config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }
}
