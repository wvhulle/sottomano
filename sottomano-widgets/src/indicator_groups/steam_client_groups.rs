mod steam_downloads_group;
mod steam_library_group;
mod steam_running_game_group;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::steam_client_pages::{SteamClientPage, open_steam_client_page};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    steam_client_files::{
        running_steam_games::RunningSteamGames, steam_content_activity::SteamContentActivity,
        steam_library_usage::SteamLibraryUsage, steam_log_lines::SteamAppId,
    },
    theme_icons::icon_requests::{IconRequest, window_theme_icon_request},
};

use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::indicator_drawing::IndicatorIcon,
    group_menus::group_menu_view::MenuHandleBadge,
    icon_catalog::drawable_icon_catalog::IconCatalog,
    indicator_groups::indicator_group_content::{
        IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        KeyedExpansions,
    },
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
pub enum SteamClientMessage {
    RunningGames(SnapshotDelivery<RunningSteamGames>),
    ContentActivity(SnapshotDelivery<SteamContentActivity>),
    LibraryUsage(SnapshotDelivery<SteamLibraryUsage>),
}

#[derive(Debug, Clone, Copy)]
pub enum SteamClientInteraction {
    OpenPage(SteamClientPage),
    RunningGameExpansion {
        app_number: u32,
        expansion: IndicatorGroupExpansionMessage,
    },
    DownloadsExpansion(IndicatorGroupExpansionMessage),
    LibraryExpansion(IndicatorGroupExpansionMessage),
}

#[derive(Default)]
pub struct SteamClientGroups {
    running_games: SubscribedSnapshot<RunningSteamGames>,
    content_activity: SubscribedSnapshot<SteamContentActivity>,
    library_usage: SubscribedSnapshot<SteamLibraryUsage>,
    running_game_expansions: KeyedExpansions<u32>,
    downloads_expansion: IndicatorGroupExpansion,
    library_expansion: IndicatorGroupExpansion,
}

impl SteamClientGroups {
    pub fn collapse(&mut self) {
        self.running_game_expansions.collapse();
        self.downloads_expansion.collapse();
        self.library_expansion.collapse();
    }

    pub fn subscription() -> Subscription<SteamClientMessage> {
        Subscription::batch([
            snapshot_subscription::<RunningSteamGames>(()).map(SteamClientMessage::RunningGames),
            snapshot_subscription::<SteamContentActivity>(())
                .map(SteamClientMessage::ContentActivity),
            snapshot_subscription::<SteamLibraryUsage>(()).map(SteamClientMessage::LibraryUsage),
        ])
    }

    pub fn update(
        &mut self,
        message: SteamClientMessage,
        clock: Clock,
    ) -> Task<SteamClientMessage> {
        match message {
            SteamClientMessage::RunningGames(delivery) => {
                self.running_games.receive(delivery, clock);
                self.running_game_expansions.track(
                    self.running_games
                        .get()
                        .into_iter()
                        .flat_map(|running| running.games.iter())
                        .map(|game| game.app_id.number),
                );
            }
            SteamClientMessage::ContentActivity(delivery) => {
                self.content_activity.receive(delivery, clock);
            }
            SteamClientMessage::LibraryUsage(delivery) => {
                self.library_usage.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: SteamClientInteraction) -> Task<SteamClientMessage> {
        match interaction {
            SteamClientInteraction::OpenPage(page) => logged_failure_task(
                format!("open Steam page {page:?}"),
                open_steam_client_page(page),
            ),
            SteamClientInteraction::RunningGameExpansion {
                app_number,
                expansion,
            } => {
                self.running_game_expansions.update(&app_number, expansion);
                Task::none()
            }
            SteamClientInteraction::DownloadsExpansion(expansion) => {
                self.downloads_expansion.update(expansion);
                Task::none()
            }
            SteamClientInteraction::LibraryExpansion(expansion) => {
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
    pub fn handle_badge(&self, icon_catalog: &IconCatalog) -> MenuHandleBadge {
        let running_game = self
            .running_games
            .get()
            .and_then(|running| running.games.first());
        let downloading = self.content_activity.get().is_some_and(|activity| {
            activity
                .entries
                .iter()
                .any(steam_downloads_group::is_downloading)
        });
        if let Some(running_game) = running_game {
            icon_catalog
                .drawable_icon(&steam_game_icon_request(running_game.app_id))
                .map_or(MenuHandleBadge::Unbadged, |drawable_icon| {
                    MenuHandleBadge::ImageBadged(drawable_icon.handle.clone())
                })
        } else if downloading {
            MenuHandleBadge::Badged(Phosphor::ArrowDown(Weight::Bold))
        } else {
            MenuHandleBadge::Unbadged
        }
    }

    #[must_use]
    pub fn group_contents<'groups>(
        &'groups self,
        clock: Clock,
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
                Some(steam_running_game_group::running_game_group(
                    game,
                    expansion,
                    locale,
                    steam_game_icon(icon_catalog, game.app_id),
                ))
            });
        let downloads_group = self.content_activity.get().and_then(|activity| {
            steam_downloads_group::downloads_group(
                activity,
                &self.downloads_expansion,
                clock,
                locale,
            )
        });
        let library_group = self.library_usage.get().and_then(|library_usage| {
            steam_library_group::library_group(
                library_usage,
                &self.library_expansion,
                locale,
                |app_id| steam_game_icon(icon_catalog, app_id),
            )
        });
        running_game_groups
            .chain(downloads_group)
            .chain(library_group)
            .collect()
    }
}
