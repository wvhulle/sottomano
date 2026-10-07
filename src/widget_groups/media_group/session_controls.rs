use iced::{Alignment, Element, widget::Row};

use crate::{
    config::Config,
    data::source::mpris::{Command, Media, Status},
    widgets::{
        confirm_button::ConfirmButton,
        glyph_button::GlyphButton,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        widget_state::WidgetState,
    },
};

pub struct SessionControls<'c, Message> {
    config: &'c Config,
    media: &'c Media,
    quit_button: &'c ConfirmButton,
    reveal: f32,
    on_command: fn(Command) -> Message,
    on_quit: Message,
    on_cancel_quit: Message,
}

impl<'c, Message: Clone> SessionControls<'c, Message> {
    pub fn new(
        config: &'c Config,
        media: &'c Media,
        quit_button: &'c ConfirmButton,
        reveal: f32,
        on_command: fn(Command) -> Message,
        on_quit: Message,
        on_cancel_quit: Message,
    ) -> Self {
        Self {
            config,
            media,
            quit_button,
            reveal,
            on_command,
            on_quit,
            on_cancel_quit,
        }
    }

    fn stop<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.stoppable().then(|| {
            let state = WidgetState::of_toggle(false);
            let color = state.color(&self.config.palette).scale_alpha(self.reveal);
            GlyphButton::new(
                Phosphor::Stop(state.weight()),
                self.config.font.icon_size_px,
                color,
                (self.on_command)(Command::Stop),
            )
            .into()
        })
    }

    fn quit<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.quit.then(|| {
            self.quit_button.view(
                self.config,
                self.reveal,
                false,
                Phosphor::Power(Weight::Regular),
                self.on_quit.clone(),
                self.on_cancel_quit.clone(),
            )
        })
    }

    fn stoppable(&self) -> bool {
        self.media.capabilities.stop && self.media.status != Status::Stopped
    }

    pub fn is_empty(&self) -> bool {
        !self.stoppable() && !self.media.capabilities.quit
    }
}

impl<'a, Message: 'a + Clone> From<SessionControls<'_, Message>> for Element<'a, Message> {
    fn from(controls: SessionControls<'_, Message>) -> Self {
        Row::with_children([controls.stop(), controls.quit()].into_iter().flatten())
            .spacing(icon_value_gap(controls.config))
            .align_y(Alignment::Center)
            .into()
    }
}
