use corredo::popovers::{CascadeOrientation, NestedMenuPresentation, nested_menu};
use sottomano_toml_config::Config;

use super::{TrayGroup, TrayMessage};
use crate::{
    group_menus::{
        cascading_layout::menu_satellite_regions::{SatelliteBlur, satellite_with_regions},
        group_menu_view::GroupMenuCascadeSurfaces,
        menu_rows::menu_row::{MenuFrames, menu_metrics, menu_style, phosphor_caret},
    },
    widgets::{
        application_menu_entries::ApplicationMenuEntries, shell_layer_appearance::LayerAppearance,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct TrayMenuLook {
    pub appearance: LayerAppearance,
    pub presentation: NestedMenuPresentation,
    pub orientation: CascadeOrientation,
}

impl TrayGroup {
    #[must_use]
    pub fn menu_surfaces_named(
        &self,
        item_name: &str,
        config: &Config,
        look: TrayMenuLook,
    ) -> Option<GroupMenuCascadeSurfaces<'static, TrayMessage>> {
        let item = self
            .items
            .get()
            .into_iter()
            .flatten()
            .find(|item| item.name() == item_name && !item.menu.is_empty())?;
        let menu_path = item.menu_path.clone()?;
        let item_address = item.address.clone();
        let on_activate = move |id| TrayMessage::MenuItemClicked {
            address: item_address.clone(),
            menu_path: menu_path.clone(),
            id,
        };
        let entries = ApplicationMenuEntries {
            config,
            pixmap_handles: &self.pixmap_handles,
            on_activate: &on_activate,
        }
        .entries(&item.menu);
        let TrayMenuLook {
            appearance,
            presentation,
            orientation,
        } = look;
        let satellite_blur = SatelliteBlur {
            item_list: appearance.backdrop_blur,
            beside: appearance.backdrop_blur,
        };
        let surfaces = nested_menu(entries, &self.menu_state, presentation, TrayMessage::Menu)
            .metrics(menu_metrics(config))
            .orientation(orientation)
            .caret(phosphor_caret(config))
            .style(menu_style(
                config,
                MenuFrames {
                    list_frame: appearance.frame,
                    beside_frame: appearance.frame,
                },
                presentation.submenu_connector,
            ))
            .surfaces()?;
        Some(GroupMenuCascadeSurfaces {
            primary: surfaces.primary,
            satellites: surfaces
                .satellites
                .into_iter()
                .map(|satellite| satellite_with_regions(satellite, satellite_blur))
                .collect(),
        })
    }
}
