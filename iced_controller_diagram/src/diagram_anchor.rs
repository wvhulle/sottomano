#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagramButton {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    LeftTrigger,
    RightTrigger,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl DiagramButton {
    pub const ALL: [DiagramButton; 17] = [
        DiagramButton::South,
        DiagramButton::East,
        DiagramButton::West,
        DiagramButton::North,
        DiagramButton::Back,
        DiagramButton::Guide,
        DiagramButton::Start,
        DiagramButton::LeftStick,
        DiagramButton::RightStick,
        DiagramButton::LeftShoulder,
        DiagramButton::RightShoulder,
        DiagramButton::LeftTrigger,
        DiagramButton::RightTrigger,
        DiagramButton::DpadUp,
        DiagramButton::DpadDown,
        DiagramButton::DpadLeft,
        DiagramButton::DpadRight,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagramStick {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagramAnchor {
    Button(DiagramButton),
    Stick(DiagramStick),
}

impl From<DiagramButton> for DiagramAnchor {
    fn from(button: DiagramButton) -> Self {
        DiagramAnchor::Button(button)
    }
}

impl From<DiagramStick> for DiagramAnchor {
    fn from(stick: DiagramStick) -> Self {
        DiagramAnchor::Stick(stick)
    }
}
