use iced::{Element, border::Radius, time::Duration};
use sottomano_config::Config;

use crate::widgets::{
    cascade_row::{CascadeDirection, CascadeOrientation},
    group_menu::{cascading_columns::cascade_offset, menu_row::menu_row_height},
    notched_panel::{NotchPlacement, NotchSide, PanelGrowth, notched_corners, notched_panel},
    panel_frame::LayerFrameStyle,
    shell_layer_appearance::LayerAppearance,
};

pub(super) struct SatellitePanels {
    notch_side: NotchSide,
    growth: PanelGrowth,
    frame: LayerFrameStyle,
    blurred: bool,
    morph: Duration,
    pub frame_corners: Radius,
    notch_depth: f32,
}

impl SatellitePanels {
    pub(super) fn new(
        appearance: &LayerAppearance,
        config: &Config,
        orientation: CascadeOrientation,
    ) -> Self {
        Self {
            growth: orientation.growth,
            notch_side: match orientation.direction {
                CascadeDirection::Rightward => NotchSide::Left,
                CascadeDirection::Leftward => NotchSide::Right,
            },
            frame: LayerFrameStyle::new(appearance, config),
            blurred: appearance.blurred,
            morph: config.tooltips.popup_morph,
            frame_corners: Radius::from(appearance.corner_radius_px),
            notch_depth: config.layout.overlay.panel_gap,
        }
    }

    pub(super) fn corners_for_rows(&self, config: &Config, rows: usize) -> Radius {
        notched_corners(
            self.notch_side,
            self.frame_corners.top_right,
            menu_row_height(config) * rows as f32,
        )
    }

    pub(super) fn beside_row<'a, Message: 'a>(
        &self,
        config: &Config,
        content: Element<'a, Message>,
        rows_above: usize,
    ) -> Element<'a, Message> {
        notched_panel(
            content,
            self.frame,
            self.blurred,
            NotchPlacement {
                side: self.notch_side,
                growth: self.growth,
                row_top: cascade_offset(config, rows_above),
                row_height: menu_row_height(config),
                notch_depth: self.notch_depth,
            },
            self.morph,
        )
    }
}
