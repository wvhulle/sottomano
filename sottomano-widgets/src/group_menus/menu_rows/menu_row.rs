use corredo::{
    popovers::{
        CaretPointing, CascadeOrientation, MenuEntry, MenuRowMetrics, NestedMenuAction,
        NestedMenuPresentation, NestedMenuState, NestedMenuStyle, NestedMenuSurfaces,
        SubmenuConnector, nested_menu,
    },
    selection_scroll::ScrollPacing,
    theming::FrameStyle,
};
use iced::{Color, Element, Theme};
use sottomano_toml_config::Config;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_menus::cascading_layout::menu_satellite_regions::{
        SatelliteBlur, satellite_with_regions,
    },
    hover_cards::hover_card_surfaces::SWITCH_INTENT,
    widgets::{shell_layer_appearance::LayerAppearance, text::resolved_text_face},
};

pub(crate) fn menu_metrics(config: &Config) -> MenuRowMetrics {
    MenuRowMetrics {
        face: resolved_text_face(&config.theme.font),
        row_height_px: config.desktop_panels.height + config.theme.spacing.item_gap,
        icon_slot_px: config.desktop_panels.height,
        item_spacing_px: config.theme.spacing.group_gap,
        side_inset_px: config.theme.spacing.surface_inset,
        indent_step_px: config.theme.font.icon_size_px,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuFrames {
    pub list_frame: FrameStyle,
    pub beside_frame: FrameStyle,
}

pub(crate) fn menu_style(
    config: &Config,
    frames: MenuFrames,
    submenu_connector: SubmenuConnector,
) -> NestedMenuStyle {
    NestedMenuStyle {
        list_frame: frames.list_frame,
        beside_frame: frames.beside_frame,
        morph: config.overlay_surfaces.surface_morph_duration,
        gap_to_parent_px: config.theme.spacing.item_gap,
        submenu_connector,
        fade_extent_px: config.group_menus.scroll_fade_rows * menu_metrics(config).row_height_px,
        reveal_delay: SWITCH_INTENT,
    }
}

pub(crate) fn phosphor_caret<'view, Message: 'view>(
    config: &Config,
) -> impl Fn(CaretPointing, fn(&Theme) -> Color) -> Element<'view, Message> + 'view {
    let icon_size_px = config.theme.font.icon_size_px;
    move |pointing, color| {
        let caret = match pointing {
            CaretPointing::Rightward => Phosphor::CaretRight(Weight::Regular),
            CaretPointing::Leftward => Phosphor::CaretLeft(Weight::Regular),
            CaretPointing::Downward => Phosphor::CaretDown(Weight::Regular),
        };
        IconGlyph::from(caret).view(icon_size_px, color)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MenuAppearance {
    pub list_appearance: LayerAppearance,
    pub beside_appearance: LayerAppearance,
    pub presentation: NestedMenuPresentation,
    pub orientation: CascadeOrientation,
}

pub fn configured_nested_menu<'view, SubmenuKey, Message>(
    entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
    menu_state: &NestedMenuState<SubmenuKey>,
    on_action: impl Fn(NestedMenuAction<SubmenuKey>) -> Message + 'view,
    config: &Config,
    appearance: MenuAppearance,
) -> Option<NestedMenuSurfaces<'view, Message>>
where
    SubmenuKey: Clone + PartialEq + 'view,
    Message: Clone + 'view,
{
    let MenuAppearance {
        list_appearance,
        beside_appearance,
        presentation,
        orientation,
    } = appearance;
    let satellite_blur = SatelliteBlur {
        item_list: list_appearance.backdrop_blur,
        beside: beside_appearance.backdrop_blur,
    };
    let surfaces = nested_menu(entries, menu_state, presentation, on_action)
        .metrics(menu_metrics(config))
        .orientation(orientation)
        .caret(phosphor_caret(config))
        .style(menu_style(
            config,
            MenuFrames {
                list_frame: list_appearance.frame,
                beside_frame: beside_appearance.frame,
            },
            presentation.submenu_connector,
        ))
        .wheel_scrolling(
            config.scrolling.wheel_line_px,
            ScrollPacing::Eased(config.scrolling.wheel_easing),
        )
        .surfaces()?;
    Some(NestedMenuSurfaces {
        primary: surfaces.primary,
        satellites: surfaces
            .satellites
            .into_iter()
            .map(|satellite| satellite_with_regions(satellite, satellite_blur))
            .collect(),
    })
}
