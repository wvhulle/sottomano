use iced::{Alignment, Element, widget::Row};
use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, LoopMode, Media, PlayerSetting, QueueOrder, Status,
};
use sottomano_toml_config::Config;

use crate::{
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    hover_cards::{MouseGesture, PressAction},
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
}

struct TransportPress {
    button_name: &'static str,
    does: &'static str,
    command: Command,
}

struct TransportButton {
    glyph: Phosphor,
    tint: ReadingStateTint,
    press: TransportPress,
}

impl<'c, Message: Clone> TransportControls<'c, Message> {
    pub fn new(
        config: &'c Config,
        media: &'c Media,
        reveal: f32,
        on_command: fn(Command) -> Message,
    ) -> Self {
        Self {
            config,
            media,
            reveal,
            on_command,
        }
    }

    #[must_use]
    pub fn press_actions(&self) -> Vec<PressAction> {
        self.buttons()
            .into_iter()
            .map(|button| {
                PressAction::new(
                    MouseGesture::LeftClick,
                    format!("{}: {}", button.press.button_name, button.press.does),
                )
            })
            .collect()
    }

    fn foreground(&self) -> ReadingStateTint {
        ReadingStateTint {
            state: ReadingDisplayState::On,
            opacity: self.reveal,
        }
    }

    fn toggle(
        &self,
        icon: Phosphor,
        state: ReadingDisplayState,
        press: TransportPress,
    ) -> TransportButton {
        TransportButton {
            glyph: icon.with_weight(state.weight()),
            tint: ReadingStateTint {
                state,
                opacity: self.reveal,
            },
            press,
        }
    }

    fn shuffle(&self) -> Option<TransportButton> {
        let PlayerSetting::Controllable(queue_order) = self.media.queue_order else {
            return None;
        };
        Some(self.toggle(
            Phosphor::Shuffle(Weight::Regular),
            ReadingDisplayState::from(queue_order),
            TransportPress {
                button_name: "Shuffle",
                does: match queue_order {
                    QueueOrder::Shuffled => "play in order",
                    QueueOrder::InOrder => "shuffle the queue",
                },
                command: Command::SetQueueOrder(queue_order.flipped()),
            },
        ))
    }

    fn previous(&self) -> Option<TransportButton> {
        self.media
            .capabilities
            .go_previous
            .then(|| TransportButton {
                glyph: Phosphor::SkipBack(Weight::Fill),
                tint: self.foreground(),
                press: TransportPress {
                    button_name: "Previous",
                    does: "go to the previous track",
                    command: Command::Previous,
                },
            })
    }

    fn play_or_pause(&self) -> Option<TransportButton> {
        match self.media.status {
            Status::Playing => self.pause(),
            Status::Paused | Status::Stopped => self.play(),
        }
    }

    fn play(&self) -> Option<TransportButton> {
        self.media.capabilities.play.then(|| TransportButton {
            glyph: Phosphor::PlayCircle(Weight::Fill),
            tint: self.foreground(),
            press: TransportPress {
                button_name: "Play",
                does: "play",
                command: Command::Play,
            },
        })
    }

    fn pause(&self) -> Option<TransportButton> {
        self.media.capabilities.pause.then(|| TransportButton {
            glyph: Phosphor::PauseCircle(Weight::Fill),
            tint: self.foreground(),
            press: TransportPress {
                button_name: "Pause",
                does: "pause",
                command: Command::Pause,
            },
        })
    }

    fn next(&self) -> Option<TransportButton> {
        self.media.capabilities.go_next.then(|| TransportButton {
            glyph: Phosphor::SkipForward(Weight::Fill),
            tint: self.foreground(),
            press: TransportPress {
                button_name: "Next",
                does: "skip to the next track",
                command: Command::Next,
            },
        })
    }

    fn loop_mode(&self) -> Option<TransportButton> {
        let PlayerSetting::Controllable(mode) = self.media.loop_mode else {
            return None;
        };
        let glyph = match mode {
            LoopMode::Off | LoopMode::Playlist => Phosphor::Repeat(Weight::Regular),
            LoopMode::Track => Phosphor::RepeatOnce(Weight::Regular),
        };
        Some(self.toggle(
            glyph,
            ReadingDisplayState::from(mode),
            TransportPress {
                button_name: "Loop",
                does: match mode.cycled() {
                    LoopMode::Off => "stop looping",
                    LoopMode::Playlist => "loop the playlist",
                    LoopMode::Track => "loop this track",
                },
                command: Command::SetLoopMode(mode.cycled()),
            },
        ))
    }

    fn buttons(&self) -> Vec<TransportButton> {
        [
            self.shuffle(),
            self.previous(),
            self.play_or_pause(),
            self.next(),
            self.loop_mode(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl<'a, Message: 'a + Clone> From<TransportControls<'_, Message>> for Element<'a, Message> {
    fn from(controls: TransportControls<'_, Message>) -> Self {
        let primary_size = controls.config.theme.font.icon_size_px;
        let button_views = controls.buttons().into_iter().map(|button| {
            let tint = button.tint;
            glyph_button(
                button.glyph,
                primary_size,
                move |theme| tint.color(theme),
                (controls.on_command)(button.press.command),
            )
        });
        Row::with_children(button_views)
            .spacing(controls.config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }
}
