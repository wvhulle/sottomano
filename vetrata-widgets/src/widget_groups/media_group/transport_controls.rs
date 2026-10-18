use iced::{Alignment, Color, Element, widget::Row};
use vetrata_config::Config;
use vetrata_sources::mpris::{Command, LoopMode, Media, Status};

use crate::{
    hover_tooltips::{PressHints, TooltipMessage, hoverable},
    widget_groups::media_group::MediaTarget,
    widgets::{
        glyph_button,
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        widget_state::WidgetState,
    },
};

pub struct TransportControls<'c, Message> {
    config: &'c Config,
    media: &'c Media,
    reveal: f32,
    on_command: fn(Command) -> Message,
    on_tooltip: fn(TooltipMessage) -> Message,
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
        on_tooltip: fn(TooltipMessage) -> Message,
    ) -> Self {
        Self {
            config,
            media,
            reveal,
            on_command,
            on_tooltip,
        }
    }

    fn primary_size(&self) -> f32 {
        self.config.theme.font.icon_size_px
    }

    fn foreground(&self) -> Color {
        WidgetState::On
            .color(&PaletteColors::from(&self.config.theme.palette))
            .scale_alpha(self.reveal)
    }

    fn toggle<'a>(
        &self,
        icon: Phosphor,
        active: bool,
        press: TransportPress,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        let state = WidgetState::of_toggle(active);
        self.control(
            icon.with_weight(state.weight()),
            state
                .color(&PaletteColors::from(&self.config.theme.palette))
                .scale_alpha(self.reveal),
            press,
        )
    }

    fn control<'a>(
        &self,
        glyph: Phosphor,
        color: Color,
        press: TransportPress,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        hoverable(
            glyph_button(
                glyph,
                self.primary_size(),
                color,
                (self.on_command)(press.command),
            ),
            press.target.into(),
            PressHints {
                left: Some(press.does),
                ..PressHints::default()
            },
            self.on_tooltip,
        )
    }

    fn shuffle<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        let shuffled = self.media.shuffle?;
        Some(self.toggle(
            Phosphor::Shuffle(Weight::Regular),
            shuffled,
            TransportPress {
                target: MediaTarget::Shuffle,
                does: if shuffled {
                    "play in order"
                } else {
                    "shuffle the queue"
                },
                command: Command::SetShuffle(!shuffled),
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
            mode != LoopMode::Off,
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
            .spacing(icon_value_gap(controls.config))
            .align_y(Alignment::Center)
            .into()
    }
}
