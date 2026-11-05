use std::path::PathBuf;

use iced::Size;
use iced_controller_diagram::{ControllerModel, DiagramAnchor};
use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::WindowLabel;
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use super::{Section, SectionBody};
use crate::{
    widget_groups::niri_window_group::niri_kdl_rules::NiriLayout,
    widgets::value_cells::LabeledValue,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ControllerCallouts {
    pub model: ControllerModel,
    pub callouts: Vec<DiagramCallout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagramCallout {
    pub anchor: DiagramAnchor,
    pub label: String,
    pub level: Severity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChipletCells {
    pub title: String,
    pub facts: Vec<LabeledValue>,
    pub cores: Vec<PhysicalCoreCells>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalCoreCells {
    pub threads: Vec<ThreadCell>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreadCell {
    pub fill: Ratio,
    pub level: Severity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewTile {
    pub window: u64,
    pub label: Option<WindowLabel>,
    pub favicon_key: Option<String>,
    pub icon: Option<PathBuf>,
    pub tile_size: Size,
    pub is_focused: bool,
    pub is_urgent: bool,
    pub is_floating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspacePreview {
    pub columns: Vec<Vec<PreviewTile>>,
    pub floating: Vec<PreviewTile>,
    pub niri_layout: NiriLayout,
}

impl Section {
    #[must_use]
    pub fn workspace(preview: WorkspacePreview) -> Self {
        Self::untitled(SectionBody::Workspace(preview))
    }
    pub fn core_grid(chiplets: impl IntoIterator<Item = ChipletCells>) -> Self {
        Self::untitled(SectionBody::CoreGrid(chiplets.into_iter().collect()))
    }
    pub fn controller_diagram(
        model: ControllerModel,
        callouts: impl IntoIterator<Item = DiagramCallout>,
    ) -> Self {
        Self::untitled(SectionBody::ControllerDiagram(ControllerCallouts {
            model,
            callouts: callouts.into_iter().collect(),
        }))
    }
}
