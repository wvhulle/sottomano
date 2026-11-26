mod game_processes_indicator;
mod install_size_indicator;
mod protondb_rating_indicator;
mod running_game_indicator;
mod steam_page_indicators;
mod steam_tool_indicator;

use std::cmp::Reverse;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::steam_client_pages::{SteamClientPage, open_steam_client_page};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    steam_client_files::{
        protondb_compatibility_summary::ProtonDbCompatibility,
        running_steam_games::{RunningSteamGame, RunningSteamGames},
        steam_app_kinds::SteamLibraryRole,
        steam_app_manifests::InstalledSteamApp,
        steam_library_usage::{SteamLibraryFolderUsage, SteamLibraryUsage},
        steam_log_lines::SteamAppId,
    },
    theme_icons::icon_requests::{IconRequest, window_theme_icon_request},
};
use sottomano_toml_config::Config;
use uom::si::{f64::Ratio, ratio::ratio};

use self::{
    game_processes_indicator::game_processes_indicator,
    install_size_indicator::install_size_indicator,
    protondb_rating_indicator::protondb_rating_indicator,
    running_game_indicator::running_game_indicator,
    steam_page_indicators::{controller_config_indicator, launch_indicator, store_page_indicator},
    steam_tool_indicator::tools_group,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::IndicatorIcon,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    icon_catalog::drawable_icon_catalog::IconCatalog,
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
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
    CompatibilityFetched(SnapshotDelivery<ProtonDbCompatibility>),
}

#[derive(Debug, Clone, Copy)]
pub enum SteamGamesInteraction {
    OpenPage(SteamClientPage),
    GameExpansion {
        app_number: u32,
        expansion: IndicatorGroupExpansionMessage,
    },
    ToolsExpansion(IndicatorGroupExpansionMessage),
    StartCompatibilityFetch(SteamAppId),
    StopCompatibilityFetch,
}

struct LibraryEntry<'library> {
    installed_app: &'library InstalledSteamApp,
    folder_usage: &'library SteamLibraryFolderUsage,
}

struct InstalledGame<'library> {
    installed_app: &'library InstalledSteamApp,
    folder_usage: &'library SteamLibraryFolderUsage,
    library_share: Ratio,
}

enum GameActivity<'library> {
    Installed(InstalledGame<'library>),
    Running {
        installed_game: InstalledGame<'library>,
        running_game: &'library RunningSteamGame,
    },
}

impl GameActivity<'_> {
    const fn installed_game(&self) -> &InstalledGame<'_> {
        match self {
            GameActivity::Installed(installed_game)
            | GameActivity::Running { installed_game, .. } => installed_game,
        }
    }
}

fn library_entries(library_usage: &SteamLibraryUsage) -> impl Iterator<Item = LibraryEntry<'_>> {
    library_usage
        .folders
        .iter()
        .flat_map(|folder_usage| {
            folder_usage
                .library_folder
                .installed_apps
                .iter()
                .map(move |installed_app| LibraryEntry {
                    installed_app,
                    folder_usage,
                })
        })
        .filter(|entry| entry.installed_app.size_on_disk_bytes > 0)
}

fn entries_in_role(
    library_usage: &SteamLibraryUsage,
    role: SteamLibraryRole,
) -> impl Iterator<Item = LibraryEntry<'_>> {
    library_entries(library_usage).filter(move |entry| {
        library_usage
            .app_kinds
            .installed_app_kind(entry.installed_app.app_id)
            .library_role()
            == role
    })
}

fn library_share(installed_app: &InstalledSteamApp, library_bytes: u64) -> Ratio {
    let game_bytes = installed_app.size_on_disk_bytes as f64;
    let total_bytes = library_bytes.max(1) as f64;
    Ratio::new::<ratio>((game_bytes / total_bytes).clamp(0.0, 1.0))
}

#[derive(Default)]
pub struct SteamGamesGroup {
    running_games: SubscribedSnapshot<RunningSteamGames>,
    library_usage: SubscribedSnapshot<SteamLibraryUsage>,
    game_expansions: KeyedExpansions<u32>,
    tools_expansion: IndicatorGroupExpansion,
    compatibility_summaries: HoverFetchedSnapshot<ProtonDbCompatibility>,
}

impl SteamGamesGroup {
    pub fn collapse(&mut self) {
        self.game_expansions.collapse();
        self.tools_expansion.collapse();
    }

    pub fn subscription() -> Subscription<SteamGamesMessage> {
        Subscription::batch([
            snapshot_subscription::<RunningSteamGames>(()).map(SteamGamesMessage::RunningGames),
            snapshot_subscription::<SteamLibraryUsage>(()).map(SteamGamesMessage::LibraryUsage),
        ])
    }

    fn track_game_expansions(&mut self) {
        self.game_expansions.track(
            self.library_usage
                .get()
                .into_iter()
                .flat_map(|library_usage| {
                    entries_in_role(library_usage, SteamLibraryRole::Playable)
                })
                .map(|entry| entry.installed_app.app_id.number),
        );
    }

    pub fn update(&mut self, message: SteamGamesMessage, clock: Clock) -> Task<SteamGamesMessage> {
        match message {
            SteamGamesMessage::RunningGames(delivery) => {
                self.running_games.receive(delivery, clock);
            }
            SteamGamesMessage::LibraryUsage(delivery) => {
                self.library_usage.receive(delivery, clock);
                self.track_game_expansions();
            }
            SteamGamesMessage::CompatibilityFetched(delivery) => {
                self.compatibility_summaries.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: SteamGamesInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<SteamGamesMessage> {
        match interaction {
            SteamGamesInteraction::OpenPage(page) => logged_failure_task(
                format!("open Steam page {page:?}"),
                open_steam_client_page(page),
            ),
            SteamGamesInteraction::GameExpansion {
                app_number,
                expansion,
            } => {
                self.game_expansions.update(&app_number, expansion);
                Task::none()
            }
            SteamGamesInteraction::ToolsExpansion(expansion) => {
                self.tools_expansion.update(expansion);
                Task::none()
            }
            SteamGamesInteraction::StartCompatibilityFetch(app_id) => {
                self.compatibility_summaries.start_hover_fetch(
                    app_id,
                    SteamGamesMessage::CompatibilityFetched,
                    config,
                    clock,
                )
            }
            SteamGamesInteraction::StopCompatibilityFetch => {
                self.compatibility_summaries.stop();
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn icon_requests(&self) -> Vec<IconRequest> {
        self.library_usage
            .get()
            .into_iter()
            .flat_map(|library_usage| entries_in_role(library_usage, SteamLibraryRole::Playable))
            .map(|entry| steam_game_icon_request(entry.installed_app.app_id))
            .collect()
    }

    #[must_use]
    pub fn running_game_icon_request(&self) -> Option<IconRequest> {
        let running_game = self.running_games.get()?.games.first()?;
        Some(steam_game_icon_request(running_game.app_id))
    }

    fn game_activities<'library>(
        &'library self,
        library_usage: &'library SteamLibraryUsage,
    ) -> Vec<GameActivity<'library>> {
        let library_bytes: u64 = library_entries(library_usage)
            .map(|entry| entry.installed_app.size_on_disk_bytes)
            .sum();
        let running_games: &[RunningSteamGame] = self
            .running_games
            .get()
            .map_or(&[], |running| running.games.as_slice());
        let mut activities: Vec<GameActivity<'library>> =
            entries_in_role(library_usage, SteamLibraryRole::Playable)
                .map(|entry| {
                    let installed_game = InstalledGame {
                        installed_app: entry.installed_app,
                        folder_usage: entry.folder_usage,
                        library_share: library_share(entry.installed_app, library_bytes),
                    };
                    match running_games
                        .iter()
                        .find(|running_game| running_game.app_id == entry.installed_app.app_id)
                    {
                        Some(running_game) => GameActivity::Running {
                            installed_game,
                            running_game,
                        },
                        None => GameActivity::Installed(installed_game),
                    }
                })
                .collect();
        activities.sort_by_key(|activity| {
            let running_first = match activity {
                GameActivity::Running { .. } => 0,
                GameActivity::Installed(_) => 1,
            };
            (
                running_first,
                Reverse(activity.installed_game().installed_app.size_on_disk_bytes),
            )
        });
        activities
    }

    fn game_group<'library>(
        &'library self,
        activity: &GameActivity<'library>,
        locale: Locale,
        game_icon: IndicatorIcon,
    ) -> Option<IndicatorGroupContent<'library>> {
        let installed_app = activity.installed_game().installed_app;
        let app_id = installed_app.app_id;
        let expansion = self.game_expansions.expansions.get(&app_id.number)?;
        let shared_rows = [
            protondb_rating_indicator(app_id, &self.compatibility_summaries),
            controller_config_indicator(app_id),
            store_page_indicator(app_id),
        ];
        let (headline_indicator, rows): (GroupIndicator<'library>, Vec<GroupIndicator<'library>>) =
            match activity {
                GameActivity::Running {
                    installed_game,
                    running_game,
                } => (
                    running_game_indicator(running_game).with_icon(game_icon),
                    [
                        running_game_indicator(running_game),
                        game_processes_indicator(running_game),
                        install_size_indicator(installed_game, locale),
                    ]
                    .into_iter()
                    .chain(shared_rows)
                    .collect(),
                ),
                GameActivity::Installed(installed_game) => (
                    install_size_indicator(installed_game, locale).with_icon(game_icon),
                    [
                        install_size_indicator(installed_game, locale),
                        launch_indicator(app_id),
                    ]
                    .into_iter()
                    .chain(shared_rows)
                    .collect(),
                ),
            };
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::SteamGame,
                app_id.number.to_string(),
            ),
            IndicatorGroupTitle::Named(installed_app.name.clone().into()),
            headline_indicator,
            expansion,
            move |expansion| SteamGamesInteraction::GameExpansion {
                app_number: app_id.number,
                expansion,
            },
            IndicatorListing::Listed(rows),
        ))
    }

    #[must_use]
    pub fn group_contents<'library>(
        &'library self,
        locale: Locale,
        icon_catalog: &'library IconCatalog,
    ) -> Vec<IndicatorGroupContent<'library>> {
        let Some(library_usage) = self.library_usage.get() else {
            return Vec::new();
        };
        self.game_activities(library_usage)
            .into_iter()
            .filter_map(|activity| {
                let game_icon =
                    steam_game_icon(icon_catalog, activity.installed_game().installed_app.app_id);
                self.game_group(&activity, locale, game_icon)
            })
            .chain(tools_group(
                entries_in_role(library_usage, SteamLibraryRole::Tooling),
                &self.tools_expansion,
                locale,
            ))
            .collect()
    }
}
