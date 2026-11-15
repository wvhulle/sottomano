use corredo::press_actions::{ConfirmButton, ConfirmStage};
use iced::{Alignment, Element, widget::Row};
use sottomano_desktop_sources::media_playback_audio::mpris_players::{Command, Media, Status};
use sottomano_toml_config::Config;

use crate::{
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    hover_cards::{MouseGesture, PressAction},
    widget_groups::media_group::confirm_button::configured_confirm_button,
    widgets::{
        glyph_button,
        phosphor::{Phosphor, Weight},
    },
};

pub struct SessionMessages<Message> {
    pub command: fn(Command) -> Message,
    pub quit: Message,
    pub cancel_quit: Message,
}

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
        }
    }

    fn offers_stop(&self) -> bool {
        self.media.capabilities.stop && self.media.status != Status::Stopped
    }

    #[must_use]
    pub fn press_actions(&self) -> Vec<PressAction> {
        let stop_action = self
            .offers_stop()
            .then(|| PressAction::new(MouseGesture::LeftClick, "Stop: stop playback"));
        let quit_action = self.media.capabilities.quit.then(|| {
            PressAction::new(
                MouseGesture::LeftClick,
                match self.quit_button.stage() {
                    ConfirmStage::Armed => "Quit: confirm quitting the player",
                    ConfirmStage::Idle => "Quit: quit the player",
                },
            )
        });
        stop_action.into_iter().chain(quit_action).collect()
    }

    fn stop<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.offers_stop().then(|| {
            let state = ReadingDisplayState::Off;
            let tint = ReadingStateTint {
                state,
                opacity: self.reveal,
            };
            glyph_button(
                Phosphor::Stop(state.weight()),
                self.config.theme.font.icon_size_px,
                move |theme| tint.color(theme),
                (self.on_command)(Command::Stop),
            )
        })
    }

    fn quit<'a>(&self) -> Option<Element<'a, Message>>
    where
        Message: 'a,
    {
        self.media.capabilities.quit.then(|| {
            configured_confirm_button(
                self.quit_button,
                self.config,
                self.reveal,
                Phosphor::Power(Weight::Regular),
                self.on_quit.clone(),
                self.on_cancel_quit.clone(),
            )
        })
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
