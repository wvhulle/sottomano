use std::cmp::Ordering;

use sottomano_compositor_model::{
    compositor_window_facts::{
        ScrollingPosition, TileViewPosition, Urgency, WindowAppId, WindowFocus, WindowPid,
        WindowTiling, WindowTitle,
    },
    compositor_window_state::{CompositorWindow, CompositorWindowLayout},
};

use crate::desktop_windows_apps::{
    terminal_emulator_windows::WindowProcesses,
    window_title_labels::{IconCandidate, WindowCaption, WindowGroupKey, WindowLabel},
};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum WorkspacePlacement {
    ScrollingTile {
        column: usize,
        tile: usize,
    },
    ScreenPosition {
        horizontal: f64,
        vertical: f64,
    },
    #[default]
    Unplaced,
}

impl From<&CompositorWindowLayout> for WorkspacePlacement {
    fn from(layout: &CompositorWindowLayout) -> Self {
        match (layout.scrolling_position, layout.tile_position_in_view) {
            (ScrollingPosition::Tile { column, tile }, _) => {
                WorkspacePlacement::ScrollingTile { column, tile }
            }
            (
                ScrollingPosition::OutsideScrolling,
                TileViewPosition::InView {
                    horizontal,
                    vertical,
                },
            ) => WorkspacePlacement::ScreenPosition {
                horizontal,
                vertical,
            },
            (ScrollingPosition::OutsideScrolling, TileViewPosition::OutsideView) => {
                WorkspacePlacement::Unplaced
            }
        }
    }
}

impl WorkspacePlacement {
    fn reading_rank(self) -> u8 {
        match self {
            WorkspacePlacement::ScrollingTile { .. } => 0,
            WorkspacePlacement::ScreenPosition { .. } => 1,
            WorkspacePlacement::Unplaced => 2,
        }
    }

    pub(super) fn compare_reading_order(&self, other: &Self) -> Ordering {
        match (self, other) {
            (
                WorkspacePlacement::ScrollingTile {
                    column: left_column,
                    tile: left_tile,
                },
                WorkspacePlacement::ScrollingTile {
                    column: right_column,
                    tile: right_tile,
                },
            ) => (left_column, left_tile).cmp(&(right_column, right_tile)),
            (
                WorkspacePlacement::ScreenPosition {
                    horizontal: left_horizontal,
                    vertical: left_vertical,
                },
                WorkspacePlacement::ScreenPosition {
                    horizontal: right_horizontal,
                    vertical: right_vertical,
                },
            ) => left_horizontal
                .total_cmp(right_horizontal)
                .then_with(|| left_vertical.total_cmp(right_vertical)),
            _ => self.reading_rank().cmp(&other.reading_rank()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WinLayout {
    pub placement: WorkspacePlacement,
    pub tile_size: (f64, f64),
    pub tiling: WindowTiling,
}

impl WinLayout {
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        match self.placement {
            WorkspacePlacement::ScrollingTile { column, .. } => Some(column),
            WorkspacePlacement::ScreenPosition { .. } | WorkspacePlacement::Unplaced => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WinEntry {
    pub id: u64,
    pub app_id: WindowAppId,
    pub pid: WindowPid,
    pub focus: WindowFocus,
    pub urgency: Urgency,
    pub layout: WinLayout,
    pub caption: WindowCaption,
}

impl WinEntry {
    pub fn icon_candidates(&self) -> Vec<IconCandidate> {
        self.caption
            .theme_icon()
            .map(IconCandidate::Theme)
            .into_iter()
            .chain(match &self.app_id {
                WindowAppId::Reported(app_id) => Some(IconCandidate::AppId(app_id.clone())),
                WindowAppId::Unreported => None,
            })
            .collect()
    }

    #[must_use]
    pub fn group_key(&self) -> Option<WindowGroupKey<'_>> {
        self.caption.group_key().or(match &self.app_id {
            WindowAppId::Reported(app_id) => Some(WindowGroupKey::Application(app_id)),
            WindowAppId::Unreported => None,
        })
    }
}

pub(super) fn win_entry(window: &CompositorWindow, processes: &WindowProcesses) -> WinEntry {
    let caption = match processes.terminal_label_in(window.id) {
        Some(terminal_label) => {
            WindowCaption::Labelled(WindowLabel::TerminalShell(terminal_label.clone()))
        }
        None => match (&window.app_id, &window.title) {
            (WindowAppId::Reported(app_id), WindowTitle::Titled(title)) => {
                WindowCaption::Labelled(WindowLabel::classify(app_id, title))
            }
            (WindowAppId::Reported(app_id), WindowTitle::Untitled) => {
                WindowCaption::Labelled(WindowLabel::classify(app_id, app_id))
            }
            (WindowAppId::Unreported, WindowTitle::Titled(title)) => {
                WindowCaption::Labelled(WindowLabel::Titled(title.clone()))
            }
            (WindowAppId::Unreported, WindowTitle::Untitled) => WindowCaption::Unlabelled,
        },
    };
    WinEntry {
        id: window.id,
        app_id: window.app_id.clone(),
        pid: window.pid,
        focus: window.focus,
        urgency: window.urgency,
        caption,
        layout: WinLayout {
            placement: WorkspacePlacement::from(&window.layout),
            tile_size: window.layout.tile_size,
            tiling: window.tiling,
        },
    }
}
