//! A self-polling holder for one [`Source`] reading: owns the latest data,
//! its poll timer (cadence declared by the source itself via
//! [`Source::INTERVAL`]), and the off-thread poll task. Groups compose cells
//! and keep only their visuals.

use iced::{Subscription, Task};

use crate::{data_sources::Source, message_subscriptions::poll};

pub struct SourceCell<S> {
    data: Option<S>,
}

impl<S> Default for SourceCell<S> {
    fn default() -> Self {
        Self { data: None }
    }
}

#[derive(Debug, Clone)]
pub enum SourceMessage<S> {
    Loaded(Option<S>),
    PollElapsed,
}

impl<S: Source + Send + 'static> SourceCell<S> {
    pub fn init() -> Task<SourceMessage<S>> {
        poll::<S>().map(SourceMessage::Loaded)
    }

    pub fn subscription() -> Subscription<SourceMessage<S>> {
        iced::time::every(S::INTERVAL).map(|_| SourceMessage::PollElapsed)
    }

    pub fn update(&mut self, msg: SourceMessage<S>) -> Task<SourceMessage<S>> {
        match msg {
            SourceMessage::Loaded(data) => {
                self.data = data;
                Task::none()
            }
            SourceMessage::PollElapsed => Self::init(),
        }
    }

    pub fn get(&self) -> Option<&S> {
        self.data.as_ref()
    }
}
