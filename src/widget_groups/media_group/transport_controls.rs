use iced::{Alignment, Color, Element, widget::Row};

use crate::{
    config::Config,
    data::source::mpris::{Command, LoopMode, Media, Setting, Status},
    widgets::{
        glyph_button::GlyphButton,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        widget_state::WidgetState,
    },
};

const SECONDARY_CONTROL_SCALE: f32 = 0.8;

pub struct TransportControls<'c, Message> {
    config: &'c Config,
    media: &'c Media,
    reveal: f32,
    on_command: fn(Command) -> Message,
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

    fn primary_size(&self) -> f32 {
        self.config.font.icon_size_px
    }

    fn secondary_size(&self) -> f32 {
        self.primary_size() * SECONDARY_CONTROL_SCALE
    }

    fn toggle_color(&self, active: bool) -> Color {
        WidgetState::of_toggle(active)
            .color(&self.config.palette)
            .scale_alpha(self.reveal)
    }

    fn foreground(&self) -> Color {
        self.toggle_color(true)
    }

    fn control<'a>(
        &self,
        glyph: Phosphor,
        size: f32,
        color: Color,
        command: Command,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        GlyphButton::new(glyph, size, color, (self.on_command)(command)).into()
    }

    fn shuffle<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        let Setting::Supported(shuffled) = self.media.shuffle else {
            return None;
        };
        Some(self.control(
            Phosphor::Shuffle(Weight::Regular),
            self.secondary_size(),
            self.toggle_color(shuffled),
            Command::SetShuffle(!shuffled),
        ))
    }

    fn previous<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.go_previous.then(|| {
            self.control(
                Phosphor::SkipBack(Weight::Fill),
                self.secondary_size(),
                self.foreground(),
                Command::Previous,
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
                self.primary_size(),
                self.foreground(),
                Command::Play,
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
                self.primary_size(),
                self.foreground(),
                Command::Pause,
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
                self.secondary_size(),
                self.foreground(),
                Command::Next,
            )
        })
    }

    fn loop_mode<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        let Setting::Supported(mode) = self.media.loop_mode else {
            return None;
        };
        let glyph = match mode {
            LoopMode::Off | LoopMode::Playlist => Phosphor::Repeat(Weight::Regular),
            LoopMode::Track => Phosphor::RepeatOnce(Weight::Regular),
        };
        Some(self.control(
            glyph,
            self.secondary_size(),
            self.toggle_color(mode != LoopMode::Off),
            Command::SetLoopMode(mode.cycled()),
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
