use iced::{Element, border::Radius, time::Duration};
use sottomano_config::Config;

use crate::{
    desktop_panels::panel_frame::LayerFrameStyle,
    group_menus::{
        cascading_layout::{
            cascade_row::{CascadeDirection, CascadeOrientation},
            cascading_columns::cascade_offset,
            notched_panel::{NotchPlacement, NotchSide, PanelGrowth, notched_panel},
        },
        menu_rows::menu_row::menu_row_height,
    },
    widgets::shell_layer_appearance::LayerAppearance,
};

pub(crate) struct CascadeSatellitePanels {
    notch_side: NotchSide,
    growth: PanelGrowth,
    frame: LayerFrameStyle,
    blurred: bool,
    morph: Duration,
    pub frame_corners: Radius,
    notch_depth: f32,
}

impl CascadeSatellitePanels {
    pub(crate) fn new(
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

    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn item_row_top(
        &self,
        config: &Config,
        member: usize,
        position: usize,
        item_count: usize,
    ) -> f32 {
        let pitch = menu_row_height(config);
        let member_row_top = cascade_offset(config, member);
        match self.growth {
            PanelGrowth::Downward => member_row_top + pitch * position as f32,
            PanelGrowth::Upward => {
                member_row_top + pitch * (position as f32 + 1.0 - item_count as f32)
            }
        }
    }

    pub(crate) fn beside_row<'a, Message: 'a>(
        &self,
        config: &Config,
        content: Element<'a, Message>,
        row_top: f32,
    ) -> Element<'a, Message> {
        notched_panel(
            content,
            self.frame,
            self.blurred,
            NotchPlacement {
                side: self.notch_side,
                growth: self.growth,
                row_top,
                row_height: menu_row_height(config),
                notch_depth: self.notch_depth,
            },
            self.morph,
        )
    }
}
