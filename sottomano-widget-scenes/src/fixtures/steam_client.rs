use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    steam_client_files::{
        protondb_compatibility_summary::{
            ProtonDbCompatibility, ProtonDbSummary, ProtonDbTier, ReportConfidence,
        },
        steam_app_kinds::SteamAppKinds,
        steam_app_manifests::{InstalledSteamApp, SteamLibraryFolder},
        steam_content_activity::{SteamContentActivity, SteamContentEntry},
        steam_library_usage::{SteamLibraryFolderUsage, SteamLibraryUsage},
        steam_log_lines::SteamAppId,
    },
};
use sottomano_widgets::{
    clock::Clock,
    icon_catalog::drawable_icon_catalog::IconCatalog,
    panel_menus::gaming_menu::{
        GamingMenu,
        steam_downloads_group::SteamDownloadsMessage,
        steam_games_group::{SteamGamesInteraction, SteamGamesMessage},
    },
};
use uom::si::{f64::InformationRate, information_rate::megabit_per_second};

use crate::showcased_group::{ShowcasedGamingMenu, ShowcasedGroup};

const LIBRARY_FOLDER: &str = "/home/user/.local/share/Steam";
pub const INSTALLED_GAME_NAME: &str = "Stardew Valley";
pub const DOWNLOADING_GAME_NAME: &str = "Hades II";

fn library_app(
    app_number: u32,
    name: &str,
    size: ByteSize,
    to_download: ByteSize,
    downloaded: ByteSize,
) -> InstalledSteamApp {
    InstalledSteamApp {
        app_id: SteamAppId { number: app_number },
        name: name.to_owned(),
        install_directory: PathBuf::from(LIBRARY_FOLDER)
            .join("steamapps/common")
            .join(name),
        size_on_disk_bytes: size.as_u64(),
        bytes_to_download: to_download.as_u64(),
        bytes_downloaded: downloaded.as_u64(),
    }
}

fn installed_game() -> InstalledSteamApp {
    library_app(
        413_150,
        INSTALLED_GAME_NAME,
        ByteSize::mib(640),
        ByteSize::b(0),
        ByteSize::b(0),
    )
}

fn downloading_game() -> InstalledSteamApp {
    library_app(
        1_145_350,
        DOWNLOADING_GAME_NAME,
        ByteSize::gib(4),
        ByteSize::gib(11),
        ByteSize::gib(4),
    )
}

fn polled_every(seconds: u64) -> DeliveryCadence {
    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(seconds)))
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut gaming_menu = GamingMenu::default();
    let _ = gaming_menu.steam_downloads_group.update(
        SteamDownloadsMessage::ContentActivity(SnapshotDelivery::fixture_delivered(
            Ok(SteamContentActivity {
                download_rate: InformationRate::new::<megabit_per_second>(184.0),
                entries: vec![SteamContentEntry::Downloading(downloading_game())],
            }),
            polled_every(2),
        )),
        clock,
    );
    let _ = gaming_menu.steam_games_group.update(
        SteamGamesMessage::LibraryUsage(SnapshotDelivery::fixture_delivered(
            Ok(SteamLibraryUsage {
                folders: vec![SteamLibraryFolderUsage {
                    library_folder: SteamLibraryFolder {
                        folder_path: PathBuf::from(LIBRARY_FOLDER),
                        installed_apps: vec![installed_game()],
                    },
                    free_bytes: ByteSize::gib(212).as_u64(),
                }],
                app_kinds: SteamAppKinds::default(),
            }),
            polled_every(30),
        )),
        clock,
    );
    ShowcasedGroup::Gaming(Box::new(ShowcasedGamingMenu {
        menu: gaming_menu,
        icon_catalog: IconCatalog::default(),
    }))
}

pub fn on_hover(started_fetch: &SteamGamesInteraction) -> Vec<SteamGamesMessage> {
    let SteamGamesInteraction::StartCompatibilityFetch(_) = started_fetch else {
        return Vec::new();
    };
    vec![SteamGamesMessage::CompatibilityFetched(
        SnapshotDelivery::single_read(Ok(ProtonDbCompatibility::Rated(ProtonDbSummary {
            tier: ProtonDbTier::Platinum,
            best_reported_tier: ProtonDbTier::Platinum,
            trending_tier: ProtonDbTier::Platinum,
            confidence: ReportConfidence::Strong,
            score: 0.91,
            total: 412,
        }))),
    )]
}
