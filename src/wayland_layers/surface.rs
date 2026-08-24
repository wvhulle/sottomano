use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, NewLayerShellSettings, OutputOption,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margin {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Margin {
    pub fn uniform(px: i32) -> Self {
        Self {
            top: px,
            right: px,
            bottom: px,
            left: px,
        }
    }

    pub fn edges(self) -> (i32, i32, i32, i32) {
        (self.top, self.right, self.bottom, self.left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub(crate) const ALL: [Position; 2] = [Position::Top, Position::Bottom];

    pub(crate) fn edge(self) -> Anchor {
        match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
        }
    }

    pub(crate) fn anchor(self) -> Anchor {
        self.edge() | Anchor::Left | Anchor::Right
    }
}

pub(crate) fn base_settings(output: Option<&str>, namespace: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output.map_or(OutputOption::Active, |name| {
            OutputOption::OutputName(name.to_string())
        }),
        namespace: Some(namespace.to_string()),
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub position: Position,
}
