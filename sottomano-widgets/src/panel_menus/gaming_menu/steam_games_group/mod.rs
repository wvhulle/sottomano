mod library_group;
mod running_game_group;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::steam_client_pages::{SteamClientPage, open_steam_client_page};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    steam_client_files::{
        running_steam_games::RunningSteamGames, steam_library_usage::SteamLibraryUsage,
        steam_log_lines::SteamAppId,
    },
    theme_icons::icon_requests::{IconRequest, window_theme_icon_request},
};

use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        indicator_drawing::IndicatorIcon,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            KeyedExpansions,
        },
    },
    icon_catalog::drawable_icon_catalog::IconCatalog,
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

fn steam_game_icon_request(app_id: SteamAppId) -> IconRequest {
    window_theme_icon_request(&format!("steam_icon_{}", app_id.number))
}

fn steam_game_icon(icon_catalog: &IconCatalog, app_id: SteamAppId) -> IndicatorIcon {
    icon_catalog
        .drawable_icon(&steam_game_icon_request(app_id))
        .map_or(IndicatorIcon::Absent, |drawable_icon| {
            IndicatorIcon::Image(drawable_icon.handle.clone())
        })
}

#[derive(Debug)]
pub enum SteamGamesMessage {
    RunningGames(SnapshotDelivery<RunningSteamGames>),
    LibraryUsage(SnapshotDelivery<SteamLibraryUsage>),
}

#[derive(Debug, Clone, Copy)]
pub enum SteamGamesInteraction {
    OpenPage(SteamClientPage),
    RunningGameExpansion {
        app_number: u32,
        expansion: IndicatorGroupExpansionMessage,
    },
    LibraryExpansion(IndicatorGroupExpansionMessage),
}

#[derive(Default)]
pub struct SteamGamesGroup {
    running_games: SubscribedSnapshot<RunningSteamGames>,
    library_usage: SubscribedSnapshot<SteamLibraryUsage>,
    running_game_expansions: KeyedExpansions<u32>,
    library_expansion: IndicatorGroupExpansion,
}

impl SteamGamesGroup {
    pub fn collapse(&mut self) {
        self.running_game_expansions.collapse();
        self.library_expansion.collapse();
    }

    pub fn subscription() -> Subscription<SteamGamesMessage> {
        Subscription::batch([
            snapshot_subscription::<RunningSteamGames>(()).map(SteamGamesMessage::RunningGames),
            snapshot_subscription::<SteamLibraryUsage>(()).map(SteamGamesMessage::LibraryUsage),
        ])
    }

    pub fn update(&mut self, message: SteamGamesMessage, clock: Clock) -> Task<SteamGamesMessage> {
        match message {
            SteamGamesMessage::RunningGames(delivery) => {
                self.running_games.receive(delivery, clock);
                self.running_game_expansions.track(
                    self.running_games
                        .get()
                        .into_iter()
                        .flat_map(|running| running.games.iter())
                        .map(|game| game.app_id.number),
                );
            }
            SteamGamesMessage::LibraryUsage(delivery) => {
                self.library_usage.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: SteamGamesInteraction) -> Task<SteamGamesMessage> {
        match interaction {
            SteamGamesInteraction::OpenPage(page) => logged_failure_task(
                format!("open Steam page {page:?}"),
                open_steam_client_page(page),
            ),
            SteamGamesInteraction::RunningGameExpansion {
                app_number,
                expansion,
            } => {
                self.running_game_expansions.update(&app_number, expansion);
                Task::none()
            }
            SteamGamesInteraction::LibraryExpansion(expansion) => {
                self.library_expansion.update(expansion);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn icon_requests(&self) -> Vec<IconRequest> {
        let running_app_ids = self
            .running_games
            .get()
            .into_iter()
            .flat_map(|running| running.games.iter())
            .map(|game| game.app_id);
        let library_app_ids = self
            .library_usage
            .get()
            .into_iter()
            .flat_map(|library_usage| library_usage.folders.iter())
            .flat_map(|folder_usage| folder_usage.library_folder.installed_apps.iter())
            .map(|installed_app| installed_app.app_id);
        running_app_ids
            .chain(library_app_ids)
            .map(steam_game_icon_request)
            .collect()
    }

    #[must_use]
    pub fn running_game_icon_request(&self) -> Option<IconRequest> {
        let running_game = self.running_games.get()?.games.first()?;
        Some(steam_game_icon_request(running_game.app_id))
    }

    #[must_use]
    pub fn group_contents<'groups>(
        &'groups self,
        locale: Locale,
        icon_catalog: &'groups IconCatalog,
    ) -> Vec<IndicatorGroupContent<'groups>> {
        let running_game_groups = self
            .running_games
            .get()
            .into_iter()
            .flat_map(|running| running.games.iter())
            .filter_map(move |game| {
                let expansion = self
                    .running_game_expansions
                    .expansions
                    .get(&game.app_id.number)?;
                Some(running_game_group::running_game_group(
                    game,
                    expansion,
                    locale,
                    steam_game_icon(icon_catalog, game.app_id),
                ))
            });
        let library_group = self.library_usage.get().and_then(|library_usage| {
            library_group::library_group(library_usage, &self.library_expansion, locale, |app_id| {
                steam_game_icon(icon_catalog, app_id)
            })
        });
        running_game_groups.chain(library_group).collect()
    }
}
