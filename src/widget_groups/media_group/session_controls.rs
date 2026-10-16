use iced::{Alignment, Element, widget::Row};

use crate::{
    config::Config,
    data::source::mpris::{Command, Media, Status},
    desktop_shell::hover_tooltips::{PressHints, TooltipMessage, hoverable},
    widget_groups::media_group::MediaTarget,
    widgets::{
        confirm_button::ConfirmButton,
        glyph_button,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        widget_state::WidgetState,
    },
};

pub struct SessionMessages<Message> {
    pub command: fn(Command) -> Message,
    pub quit: Message,
    pub cancel_quit: Message,
    pub tooltip: fn(TooltipMessage) -> Message,
}

pub struct SessionControls<'c, Message> {
    config: &'c Config,
    media: &'c Media,
    quit_button: &'c ConfirmButton,
    reveal: f32,
    on_command: fn(Command) -> Message,
    on_quit: Message,
    on_cancel_quit: Message,
    on_tooltip: fn(TooltipMessage) -> Message,
}

impl<'c, Message: Clone> SessionControls<'c, Message> {
    pub fn new(
        config: &'c Config,
        media: &'c Media,
        quit_button: &'c ConfirmButton,
        reveal: f32,
        messages: SessionMessages<Message>,
    ) -> Self {
        Self {
            config,
            media,
            quit_button,
            reveal,
            on_command: messages.command,
            on_quit: messages.quit,
            on_cancel_quit: messages.cancel_quit,
            on_tooltip: messages.tooltip,
        }
    }

    fn stop<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.stoppable().then(|| {
            let state = WidgetState::of_toggle(false);
            let color = state
                .color(&self.config.theme.palette)
                .scale_alpha(self.reveal);
            hoverable(
                glyph_button(
                    Phosphor::Stop(state.weight()),
                    self.config.theme.font.icon_size_px,
                    color,
                    (self.on_command)(Command::Stop),
                ),
                MediaTarget::Stop.into(),
                PressHints {
                    left: Some("stop playback"),
                    ..PressHints::default()
                },
                self.on_tooltip,
            )
        })
    }

    fn quit<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.quit.then(|| {
            hoverable(
                self.quit_button.view(
                    self.config,
                    self.reveal,
                    false,
                    Phosphor::Power(Weight::Regular),
                    self.on_quit.clone(),
                    self.on_cancel_quit.clone(),
                ),
                MediaTarget::Quit.into(),
                PressHints {
                    left: Some(if self.quit_button.is_armed() {
                        "confirm quitting the player"
                    } else {
                        "quit the player"
                    }),
                    ..PressHints::default()
                },
                self.on_tooltip,
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
