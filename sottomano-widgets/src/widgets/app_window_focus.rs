use sottomano_desktop_sources::desktop_windows_apps::niri_window_placement::WindowOwner;

#[derive(Debug, Clone)]
pub struct AppWindowFocus<Unmatched> {
    pub owner: WindowOwner,
    pub unmatched: Unmatched,
}
