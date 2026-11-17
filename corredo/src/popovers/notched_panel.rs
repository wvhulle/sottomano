use iced::{
    Element, Point, Rectangle, Size, Vector, advanced::layout::Limits, border::Radius,
    time::Duration,
};

use crate::{
    orientation::Edge,
    popovers::{
        CascadeDirection, CascadeOrientation, ConnectorTriangle, PopupGrowth, RectangleMorph,
        SubmenuConnector,
    },
    theming::FrameStyle,
};

#[cfg(test)]
mod nesting_tests;
mod panel_widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotchSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotchRowAnchor {
    FromPrimaryTop(f32),
    WithinParent { row_offset: f32 },
}

impl NotchRowAnchor {
    fn row_top(self, parent_panel_top: f32) -> f32 {
        match self {
            NotchRowAnchor::FromPrimaryTop(row_top) => row_top,
            NotchRowAnchor::WithinParent { row_offset } => parent_panel_top + row_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatelliteRole {
    ItemList,
    Beside,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScreenRoom {
    pub above_primary_top: f32,
    pub below_primary_top: f32,
}

impl From<&Limits> for ScreenRoom {
    fn from(limits: &Limits) -> Self {
        let below_primary_top = limits.min().height;
        Self {
            above_primary_top: limits.max().height - below_primary_top,
            below_primary_top,
        }
    }
}

impl ScreenRoom {
    pub(crate) fn limits_within(self, width: f32) -> Limits {
        Limits::new(
            Size::new(0.0, self.below_primary_top),
            Size::new(width, self.above_primary_top + self.below_primary_top),
        )
    }

    fn panel_top_on_screen(self, wanted_top: f32, panel_height: f32) -> f32 {
        let highest_top = -self.above_primary_top;
        let lowest_top = (self.below_primary_top - panel_height).max(highest_top);
        wanted_top.clamp(highest_top, lowest_top)
    }
}

const DEFAULT_MORPH: Duration = Duration::from_millis(160);

#[derive(Debug, Clone)]
struct NotchPlacement {
    side: NotchSide,
    growth: PopupGrowth,
    row: NotchRowAnchor,
    row_height: f32,
    gap_to_parent: f32,
    submenu_connector: SubmenuConnector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotchedPanelRegions {
    pub surface_rows: Vec<Rectangle>,
    pub surface_corners: Radius,
    pub accepting_input: Rectangle,
    pub bounds: Rectangle,
    pub role: SatelliteRole,
}

pub struct NotchedPanel<'a, Message> {
    content: Element<'a, Message>,
    frame: FrameStyle,
    corner_radius: f32,
    placement: NotchPlacement,
    row_top: f32,
    morph: Duration,
    beside: Vec<NotchedPanel<'a, Message>>,
    parent_panel_top: f32,
    role: SatelliteRole,
}

#[must_use]
pub fn notched_panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    orientation: CascadeOrientation,
    row: NotchRowAnchor,
) -> NotchedPanel<'a, Message> {
    let frame = FrameStyle::default();
    NotchedPanel {
        content: content.into(),
        corner_radius: frame.corner_radius().top_left,
        frame,
        row_top: row.row_top(0.0),
        placement: NotchPlacement {
            side: match orientation.direction {
                CascadeDirection::Rightward => NotchSide::Left,
                CascadeDirection::Leftward => NotchSide::Right,
            },
            growth: orientation.growth,
            row,
            row_height: 0.0,
            gap_to_parent: 0.0,
            submenu_connector: SubmenuConnector::default(),
        },
        morph: DEFAULT_MORPH,
        beside: Vec::new(),
        parent_panel_top: 0.0,
        role: SatelliteRole::ItemList,
    }
}

impl<'a, Message: 'a> NotchedPanel<'a, Message> {
    #[must_use]
    pub fn frame(self, frame: FrameStyle) -> Self {
        Self {
            corner_radius: frame.corner_radius().top_left,
            frame,
            ..self
        }
    }

    #[must_use]
    pub fn row_height(self, row_height: f32) -> Self {
        Self {
            placement: NotchPlacement {
                row_height,
                ..self.placement
            },
            ..self
        }
    }

    #[must_use]
    pub fn gap_to_parent(self, gap_to_parent: f32) -> Self {
        Self {
            placement: NotchPlacement {
                gap_to_parent,
                ..self.placement
            },
            ..self
        }
    }

    #[must_use]
    pub fn submenu_connector(self, submenu_connector: SubmenuConnector) -> Self {
        Self {
            placement: NotchPlacement {
                submenu_connector,
                ..self.placement
            },
            ..self
        }
    }

    #[must_use]
    pub fn role(self, role: SatelliteRole) -> Self {
        Self { role, ..self }
    }

    #[must_use]
    pub fn morph(self, morph: Duration) -> Self {
        Self { morph, ..self }
    }

    #[must_use]
    pub fn beside(self, beside: impl IntoIterator<Item = NotchedPanel<'a, Message>>) -> Self {
        Self {
            beside: beside.into_iter().collect(),
            ..self
        }
    }
}

impl<'a, Message: 'a> From<NotchedPanel<'a, Message>> for Element<'a, Message> {
    fn from(notched_panel: NotchedPanel<'a, Message>) -> Self {
        Element::new(notched_panel)
    }
}

#[derive(Default)]
struct PanelMorph {
    laid_out: Rectangle,
    laid_out_folded: Rectangle,
    morphing_toward: Rectangle,
    morph: RectangleMorph,
}

impl<Message> NotchedPanel<'_, Message> {
    fn row_center(&self) -> f32 {
        self.row_top + self.placement.row_height / 2.0
    }

    fn panel_target(&self, content_size: Size, screen_room: ScreenRoom) -> Rectangle {
        let panel_x = match self.placement.side {
            NotchSide::Left => self.placement.gap_to_parent,
            NotchSide::Right => 0.0,
        };
        let wanted_top = match self.placement.growth {
            PopupGrowth::Downward => self.row_top,
            PopupGrowth::Upward => self.row_top + self.placement.row_height - content_size.height,
        };
        let panel_y = screen_room.panel_top_on_screen(wanted_top, content_size.height);
        Rectangle::new(Point::new(panel_x, panel_y), content_size)
    }

    fn folded_into_notch(&self, target: Rectangle) -> Rectangle {
        let notch_edge_x = match self.placement.side {
            NotchSide::Left => target.x,
            NotchSide::Right => target.x + target.width,
        };
        Rectangle::new(
            Point::new(notch_edge_x, self.row_center() - self.corner_radius),
            Size::new(0.0, 2.0 * self.corner_radius),
        )
    }

    fn node_origin_from_notch_edge(&self, node_width: f32) -> Vector {
        match self.placement.side {
            NotchSide::Left => Vector::ZERO,
            NotchSide::Right => Vector::new(-node_width, 0.0),
        }
    }

    fn drawn_panel(&self, panel_morph: &PanelMorph, node_bounds: Rectangle) -> Rectangle {
        let notch_edge = Vector::new(node_bounds.x, node_bounds.y)
            - self.node_origin_from_notch_edge(node_bounds.width);
        panel_morph.morph.current + notch_edge
    }

    fn panel_with_gap(&self, panel: Rectangle) -> Rectangle {
        let gap_left_x = match self.placement.side {
            NotchSide::Left => panel.x - self.placement.gap_to_parent,
            NotchSide::Right => panel.x,
        };
        Rectangle::new(
            Point::new(gap_left_x, panel.y),
            Size::new(panel.width + self.placement.gap_to_parent, panel.height),
        )
    }

    fn panel_corners(&self) -> Radius {
        Radius::from(self.corner_radius)
    }

    fn connector(&self, panel: Rectangle, origin: Vector) -> ConnectorTriangle {
        let notch_y = origin.y + self.row_center();
        let apex_x = match self.placement.side {
            NotchSide::Left => panel.x - self.placement.gap_to_parent,
            NotchSide::Right => panel.x + panel.width + self.placement.gap_to_parent,
        };
        let side = match self.placement.side {
            NotchSide::Left => Edge::Left,
            NotchSide::Right => Edge::Right,
        };
        ConnectorTriangle::between(
            Rectangle::new(Point::new(apex_x, notch_y), Size::ZERO),
            apex_x,
            panel,
            side,
            0.0,
        )
    }

    fn drawn_surface_rows(&self, panel: Rectangle, origin: Vector) -> Vec<Rectangle> {
        match self.placement.submenu_connector {
            SubmenuConnector::Shown => self
                .connector(panel, origin)
                .surface_rows(panel, self.panel_corners()),
            SubmenuConnector::Hidden => vec![panel],
        }
    }
}

#[cfg(test)]
mod screen_room_tests {
    use super::ScreenRoom;

    const ROOM_OVER_BOTTOM_PANEL: ScreenRoom = ScreenRoom {
        above_primary_top: 300.0,
        below_primary_top: 100.0,
    };

    #[test]
    fn a_panel_that_fits_stays_at_its_row() {
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(-40.0, 100.0) + 40.0).abs() < f32::EPSILON
        );
    }

    #[test]
    fn a_panel_past_either_screen_edge_slides_just_back_on_screen() {
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(-350.0, 350.0) + 300.0).abs()
                < f32::EPSILON
        );
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(40.0, 350.0) + 250.0).abs() < f32::EPSILON
        );
    }

    #[test]
    fn a_panel_taller_than_the_screen_starts_at_the_screen_top() {
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(40.0, 400.0) + 300.0).abs() < f32::EPSILON
        );
    }
}
