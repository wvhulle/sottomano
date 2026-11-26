pub mod gamepad_group;
pub mod steam_downloads_group;
pub mod steam_games_group;

use sottomano_toml_config::Config;

use self::{
    gamepad_group::GamepadGroup,
    steam_downloads_group::{SteamDownloadActivity, SteamDownloadsGroup},
    steam_games_group::SteamGamesGroup,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::group_menu_view::MenuHandleBadge,
    icon_catalog::drawable_icon_catalog::IconCatalog,
};

#[derive(Default)]
pub struct GamingMenu {
    pub gamepad_group: GamepadGroup,
    pub steam_downloads_group: SteamDownloadsGroup,
    pub steam_games_group: SteamGamesGroup,
}

impl GamingMenu {
    pub fn collapse(&mut self) {
        self.gamepad_group.collapse();
        self.steam_downloads_group.collapse();
        self.steam_games_group.collapse();
    }

    #[must_use]
    pub fn handle_badge(&self, icon_catalog: &IconCatalog) -> MenuHandleBadge {
        if let Some(running_game_icon) = self.steam_games_group.running_game_icon_request() {
            return icon_catalog
                .drawable_icon(&running_game_icon)
                .map_or(MenuHandleBadge::Unbadged, |drawable_icon| {
                    MenuHandleBadge::ImageBadged(drawable_icon.handle.clone())
                });
        }
        match self.steam_downloads_group.download_activity() {
            SteamDownloadActivity::Downloading => {
                MenuHandleBadge::Badged(Phosphor::ArrowDown(Weight::Bold))
            }
            SteamDownloadActivity::Quiet => MenuHandleBadge::Unbadged,
        }
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        icon_catalog: &'view IconCatalog,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let locale = config.calendar.locale;
        self.gamepad_group
            .group_contents(&config.panel_menus.gaming.gamepad, clock)
            .into_iter()
            .chain(self.steam_downloads_group.group_contents(clock, locale))
            .chain(self.steam_games_group.group_contents(locale, icon_catalog))
            .collect()
    }
}
