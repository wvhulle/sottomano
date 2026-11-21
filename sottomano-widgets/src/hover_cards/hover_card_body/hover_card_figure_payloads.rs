use corredo::gauges::{DurationTimeline, TimelineDensity};
use iced::{Size, Vector};
use iced_controller_diagram::{ControllerModel, DiagramAnchor};
use sottomano_compositor_model::{
    compositor_styling_model::AppWindowLayout,
    compositor_window_facts::{Urgency, WindowFocus, WindowTiling},
};
use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::WindowLabel;
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use super::SectionWidget;
use crate::{
    icon_catalog::drawable_icon_catalog::DrawableIcon, widgets::value_cells::LabeledValue,
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
    pub caption: TileCaption,
    pub icon: TileIcon,
    pub tile_size: Size,
    pub focus: WindowFocus,
    pub urgency: Urgency,
    pub tiling: WindowTiling,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TileCaption {
    Untitled,
    Titled(WindowLabel),
}

impl TileCaption {
    #[must_use]
    pub fn favicon_key(&self) -> Option<&str> {
        match self {
            TileCaption::Titled(label) => label.favicon_key(),
            TileCaption::Untitled => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TileIcon {
    Unresolved,
    Resolved(DrawableIcon),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspacePreview {
    pub arrangement: WorkspaceArrangement,
    pub app_window_layout: AppWindowLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceArrangement {
    ScrollingColumns {
        columns: Vec<Vec<PreviewTile>>,
        floating: Vec<PreviewTile>,
    },
    ScreenGeometry {
        tiles: Vec<PlacedPreviewTile>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPreviewTile {
    pub tile: PreviewTile,
    pub offset: Vector,
}

impl From<WorkspacePreview> for SectionWidget {
    fn from(preview: WorkspacePreview) -> Self {
        SectionWidget::drawn(move |card| card.workspace_schematic(&preview))
    }
}

impl From<ControllerCallouts> for SectionWidget {
    fn from(diagram: ControllerCallouts) -> Self {
        SectionWidget::drawn(move |card| card.controller_diagram(&diagram))
    }
}

pub fn core_grid_widget(chiplets: impl IntoIterator<Item = ChipletCells>) -> SectionWidget {
    let chiplets: Vec<ChipletCells> = chiplets.into_iter().collect();
    SectionWidget::drawn(move |card| card.core_grid(&chiplets))
}

pub fn controller_diagram_widget(
    model: ControllerModel,
    callouts: impl IntoIterator<Item = DiagramCallout>,
) -> SectionWidget {
    SectionWidget::from(ControllerCallouts {
        model,
        callouts: callouts.into_iter().collect(),
    })
}

#[must_use]
pub fn timeline_widget(timeline: DurationTimeline) -> SectionWidget {
    SectionWidget::drawn(move |_| {
        timeline
            .clone()
            .density(TimelineDensity::LabeledColumn)
            .view()
    })
}
