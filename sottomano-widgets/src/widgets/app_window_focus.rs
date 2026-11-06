use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::WindowOwner;

#[derive(Debug, Clone)]
pub struct AppWindowFocus<Unmatched> {
    pub owner: WindowOwner,
    pub unmatched: Unmatched,
}
