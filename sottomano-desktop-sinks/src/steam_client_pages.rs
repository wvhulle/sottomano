use sottomano_desktop_sources::{
    steam_client_files::steam_log_lines::SteamAppId, system_protocol_clients::shell_command_output,
};

use crate::finished_commands::run_to_success;

const STEAM_LAUNCHER: &str = "steam";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamClientPage {
    Downloads,
    GameDetails(SteamAppId),
    GameLaunch(SteamAppId),
    ControllerConfiguration(SteamAppId),
}

impl SteamClientPage {
    fn steam_url(self) -> String {
        match self {
            SteamClientPage::Downloads => "steam://open/downloads".to_owned(),
            SteamClientPage::GameDetails(app_id) => {
                format!("steam://nav/games/details/{}", app_id.number)
            }
            SteamClientPage::GameLaunch(app_id) => {
                format!("steam://rungameid/{}", app_id.number)
            }
            SteamClientPage::ControllerConfiguration(app_id) => {
                format!("steam://controllerconfig/{}", app_id.number)
            }
        }
    }
}

pub async fn open_steam_client_page(
    page: SteamClientPage,
) -> Result<(), shell_command_output::Error> {
    run_to_success(STEAM_LAUNCHER, [page.steam_url()]).await
}
