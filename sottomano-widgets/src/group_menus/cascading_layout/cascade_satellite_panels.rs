use iced::{Element, border::Radius, time::Duration};
use sottomano_config::Config;

use crate::{
    desktop_panels::panel_frame::LayerFrameStyle,
    group_menus::{
        cascading_layout::{
            cascade_row::{CascadeDirection, CascadeOrientation},
            notched_panel::{
                NotchPlacement, NotchRowAnchor, NotchSide, PanelGrowth, ResolvedPanelTop,
                notched_panel,
            },
        },
        menu_rows::menu_row::menu_row_height,
    },
    widgets::{edge_alpha_fade::ScrollEdgeFade, shell_layer_appearance::LayerAppearance},
};

#[derive(Clone, Copy)]
struct PanelSurface {
    frame: LayerFrameStyle,
    blurred: bool,
}

pub(crate) struct CascadeSatellitePanels {
    notch_side: NotchSide,
    pub growth: PanelGrowth,
    list_frame: LayerFrameStyle,
    list_blurred: bool,
    card_frame: LayerFrameStyle,
    card_blurred: bool,
    morph: Duration,
    pub frame_corners: Radius,
    notch_depth: f32,
}

impl CascadeSatellitePanels {
    pub(crate) fn new(
        menu_appearance: &LayerAppearance,
        card_appearance: &LayerAppearance,
        config: &Config,
        orientation: CascadeOrientation,
    ) -> Self {
        Self {
            growth: orientation.growth,
            notch_side: match orientation.direction {
                CascadeDirection::Rightward => NotchSide::Left,
                CascadeDirection::Leftward => NotchSide::Right,
            },
            list_frame: LayerFrameStyle::new(menu_appearance),
            list_blurred: menu_appearance.blurred,
            card_frame: LayerFrameStyle::new(card_appearance),
            card_blurred: card_appearance.blurred,
            morph: config.overlay_surfaces.surface_morph_duration,
            frame_corners: Radius::from(menu_appearance.corner_radius_px),
            notch_depth: config.theme.spacing.group_gap,
        }
    }

    pub(crate) fn rows_edge_fade(&self, config: &Config) -> ScrollEdgeFade {
        ScrollEdgeFade::within_frame(self.list_frame, menu_row_height(config))
    }

    pub(crate) fn beside_row<'a, Message: 'a>(
        &self,
        config: &Config,
        content: Element<'a, Message>,
        row_top: f32,
    ) -> Element<'a, Message> {
        self.anchored_panel(
            config,
            content,
            PanelSurface {
                frame: self.card_frame,
                blurred: self.card_blurred,
            },
            NotchRowAnchor::FromPrimaryTop(row_top),
            ResolvedPanelTop::default(),
        )
    }

    pub(crate) fn item_list_beside_row<'a, Message: 'a>(
        &self,
        config: &Config,
        item_list: Element<'a, Message>,
        row_top: f32,
        list_top: ResolvedPanelTop,
    ) -> Element<'a, Message> {
        self.anchored_panel(
            config,
            item_list,
            PanelSurface {
                frame: self.list_frame,
                blurred: self.list_blurred,
            },
            NotchRowAnchor::FromPrimaryTop(row_top),
            list_top,
        )
    }

    pub(crate) fn card_beside_item_row<'a, Message: 'a>(
        &self,
        config: &Config,
        card: Element<'a, Message>,
        list_top: ResolvedPanelTop,
        row_offset: f32,
    ) -> Element<'a, Message> {
        self.anchored_panel(
            config,
            card,
            PanelSurface {
                frame: self.card_frame,
                blurred: self.card_blurred,
            },
            NotchRowAnchor::WithinPanel {
                panel_top: list_top,
                row_offset,
            },
            ResolvedPanelTop::default(),
        )
    }

    fn anchored_panel<'a, Message: 'a>(
        &self,
        config: &Config,
        content: Element<'a, Message>,
        surface: PanelSurface,
        row: NotchRowAnchor,
        panel_top: ResolvedPanelTop,
    ) -> Element<'a, Message> {
        notched_panel(
            content,
            surface.frame,
            surface.blurred,
            NotchPlacement {
                side: self.notch_side,
                growth: self.growth,
                row,
                panel_top,
                row_height: menu_row_height(config),
                notch_depth: self.notch_depth,
            },
            self.morph,
        )
    }
}
