use iced::{Element, border::Radius, time::Duration};
use sottomano_toml_config::Config;

use crate::{
    desktop_panels::{panel_frame::FrameStyle, region_reporter::BackdropBlur},
    group_menus::{
        cascading_layout::{
            cascade_row::{CascadeDirection, CascadeOrientation},
            notched_panel::{
                NotchPlacement, NotchRowAnchor, NotchSide, PanelGrowth, ResolvedPanelTop,
                notched_panel,
            },
        },
        group_menu_presentation::SubmenuConnector,
        menu_rows::menu_row::menu_row_height,
    },
    widgets::{edge_alpha_fade::ScrollEdgeFade, shell_layer_appearance::LayerAppearance},
};

#[derive(Clone, Copy)]
struct PanelSurface {
    frame: FrameStyle,
    backdrop_blur: BackdropBlur,
}

pub(crate) struct CascadeSatellitePanels {
    notch_side: NotchSide,
    pub growth: PanelGrowth,
    list_frame: FrameStyle,
    list_backdrop_blur: BackdropBlur,
    card_frame: FrameStyle,
    card_backdrop_blur: BackdropBlur,
    morph: Duration,
    pub frame_corners: Radius,
    gap_to_parent: f32,
    submenu_connector: SubmenuConnector,
}

impl CascadeSatellitePanels {
    pub(crate) fn new(
        menu_appearance: &LayerAppearance,
        card_appearance: &LayerAppearance,
        config: &Config,
        orientation: CascadeOrientation,
        submenu_connector: SubmenuConnector,
    ) -> Self {
        Self {
            growth: orientation.growth,
            notch_side: match orientation.direction {
                CascadeDirection::Rightward => NotchSide::Left,
                CascadeDirection::Leftward => NotchSide::Right,
            },
            list_frame: menu_appearance.frame_style(),
            list_backdrop_blur: menu_appearance.backdrop_blur,
            card_frame: card_appearance.frame_style(),
            card_backdrop_blur: card_appearance.backdrop_blur,
            morph: config.overlay_surfaces.surface_morph_duration,
            frame_corners: Radius::from(menu_appearance.corner_radius_px),
            gap_to_parent: config.theme.spacing.item_gap,
            submenu_connector,
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
                backdrop_blur: self.card_backdrop_blur,
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
                backdrop_blur: self.list_backdrop_blur,
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
                backdrop_blur: self.card_backdrop_blur,
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
            surface.backdrop_blur,
            NotchPlacement {
                side: self.notch_side,
                growth: self.growth,
                row,
                panel_top,
                row_height: menu_row_height(config),
                gap_to_parent: self.gap_to_parent,
                submenu_connector: self.submenu_connector,
            },
            self.morph,
        )
    }
}
