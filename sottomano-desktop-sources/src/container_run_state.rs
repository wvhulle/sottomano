#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerRunState {
    Running,
    Stopped,
}

impl ContainerRunState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ContainerRunState::Running => "running",
            ContainerRunState::Stopped => "stopped",
        }
    }
}
