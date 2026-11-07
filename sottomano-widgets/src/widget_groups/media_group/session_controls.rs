use iced::{Alignment, Element, widget::Row};
use sottomano_desktop_sources::media_playback_audio::mpris_players::{Command, Media, Status};
use sottomano_toml_config::Config;

use crate::{
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    hover_cards::{HoverCardMessage, PressHints, hoverable},
    widget_groups::media_group::{
        MediaTarget, PlayerTarget,
        confirm_button::{ConfirmButton, configured_confirm_button},
    },
    widgets::{
        glyph_button,
        phosphor::{Phosphor, Weight},
    },
};

pub struct SessionMessages<Message> {
    pub command: fn(Command) -> Message,
    pub quit: Message,
    pub cancel_quit: Message,
    pub hover_card: fn(HoverCardMessage) -> Message,
}

pub struct SessionControls<'c, Message> {
    config: &'c Config,
    media: &'c Media,
    quit_button: &'c ConfirmButton,
    reveal: f32,
    on_command: fn(Command) -> Message,
    on_quit: Message,
    on_cancel_quit: Message,
    on_hover_card: fn(HoverCardMessage) -> Message,
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
            on_hover_card: messages.hover_card,
        }
    }

    fn stop<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.stoppable().then(|| {
            let state = ReadingDisplayState::Off;
            let tint = ReadingStateTint {
                state,
                opacity: self.reveal,
            };
            hoverable(
                glyph_button(
                    Phosphor::Stop(state.weight()),
                    self.config.theme.font.icon_size_px,
                    move |theme| tint.color(theme),
                    (self.on_command)(Command::Stop),
                ),
                PlayerTarget::of(&self.media.bus, MediaTarget::Stop).into(),
                PressHints {
                    left: Some("stop playback"),
                    ..PressHints::default()
                },
                self.on_hover_card,
            )
        })
    }

    fn quit<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.quit.then(|| {
            hoverable(
                configured_confirm_button(
                    self.quit_button,
                    self.config,
                    self.reveal,
                    Phosphor::Power(Weight::Regular),
                    self.on_quit.clone(),
                    self.on_cancel_quit.clone(),
                ),
                PlayerTarget::of(&self.media.bus, MediaTarget::Quit).into(),
                PressHints {
                    left: Some(if self.quit_button.is_armed() {
                        "confirm quitting the player"
                    } else {
                        "quit the player"
                    }),
                    ..PressHints::default()
                },
                self.on_hover_card,
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
            .spacing(controls.config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }
}
