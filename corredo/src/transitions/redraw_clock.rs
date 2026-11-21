use iced::time::Instant;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RedrawClock {
    #[default]
    AwaitingFirstFrame,
    FrameAt(Instant),
}
