use std::{
    mem::take,
    time::{Duration, Instant},
};

use iced::{Element, mouse::ScrollDelta};

use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::mpris::{Command, Media, SeekMode, micros},
    },
    widgets::{inline_slider::inline_slider, slider_control::scroll_direction},
};

const SCROLL_SEEK_STEP: Duration = Duration::from_secs(5);
const DRAG_STEP_SECONDS: f32 = 1.0;

#[derive(Debug, Clone, Copy)]
pub enum SeekMessage {
    Drag(f32),
    Release,
    Scroll(ScrollDelta),
}

#[derive(Debug, Default)]
enum Drag {
    #[default]
    Idle,
    Dragging(Duration),
}

#[derive(Debug, Default)]
pub struct SeekBar {
    drag: Drag,
    scroll_accumulated: f32,
}

impl SeekBar {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, message: SeekMessage, media: &Media, now: Instant) -> Option<Command> {
        match message {
            SeekMessage::Drag(seconds) => {
                self.drag = Drag::Dragging(Duration::from_secs_f32(seconds.max(0.0)));
                None
            }
            SeekMessage::Release => match take(&mut self.drag) {
                Drag::Dragging(target) => seek_to(media, target, now),
                Drag::Idle => None,
            },
            SeekMessage::Scroll(delta) => {
                let steps = scroll_direction(&mut self.scroll_accumulated, delta)?;
                Some(Command::Seek(steps * micros(SCROLL_SEEK_STEP)))
            }
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
        let length = media.progress.length()?;
        let played = media.progress.played(media.status, now)?;
        let shown = match self.drag {
            Drag::Dragging(target) => target,
            Drag::Idle => played,
        };
        Some(inline_slider(
            config,
            0.0..=length.as_secs_f32(),
            shown.as_secs_f32(),
            None,
            DRAG_STEP_SECONDS,
            Severity::Normal,
            SeekMessage::Drag,
            SeekMessage::Release,
            SeekMessage::Scroll,
        ))
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
