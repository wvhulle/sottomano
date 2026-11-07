use std::collections::{HashMap, HashSet};

use iced::Task;
use sottomano_desktop_sinks::desktop_windows_apps::compositor_requests::CompositorRequestFailure;
use sottomano_desktop_sources::desktop_windows_apps::{
    app_launch_history::LaunchScope,
    compositor_connection::{CompositorKind, WorkspaceConfigSource, missing_compositor_reason},
    compositor_output_facts::OutputInfo,
    compositor_window_state::{
        CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowState,
    },
    desktop_entry_app_catalog::{LaunchableApp, open_windows_by_app},
    desktop_entry_app_icons::{self},
    workspace_window_snapshot,
};
use sottomano_widgets::widget_groups::{
    app_launcher_group::{
        AppLaunchUsage, FrequentApp, FrequentAppScope, OutputFrequentApps, WorkspaceWindowPresence,
    },
    gamepad_group::GamepadWindowClaim,
};

use crate::{
    desktop_shell::{DesktopShell, sqlite_app_launches},
    shell_message::Message,
};

impl DesktopShell {
    pub(super) fn derive_window_snapshot(&mut self) {
        self.window_snapshot = workspace_window_snapshot::Snapshot::derive(
            &self.compositor,
            self.widget_groups.window_group.window_processes(),
        );
    }

    pub(super) fn show_compositor_failure(
        &mut self,
        failure: &CompositorRequestFailure,
    ) -> Task<Message> {
        tracing::warn!("{failure}");
        self.derive_window_snapshot();
        self.notifications.show_compositor_failure(
            failure,
            &self.widget_groups.toast_context(
                &self.compositor,
                self.desktop_entries.get(),
                &self.config,
                self.clock,
            ),
        )
    }

    pub(super) fn read_niri_workspace_order(&self) -> Task<Message> {
        if CompositorKind::detected().map(CompositorKind::workspace_config_source)
            != Some(WorkspaceConfigSource::NiriKdl)
        {
            return Task::none();
        }
        Task::perform(self.niri_config.read_workspaces(), Message::NiriConfigRead)
    }

    pub(super) fn apply_compositor_update(&mut self, update: CompositorUpdate) -> Task<Message> {
        let changed = |change| update.changes.contains(&change);
        let config_reloaded = changed(CompositorChange::ConfigReload);
        let focus_changed = changed(CompositorChange::Focus);
        let windows_changed = changed(CompositorChange::Windows);
        let workspaces_changed = changed(CompositorChange::Workspaces);
        self.compositor = update.state;
        let mut tasks: Vec<Task<Message>> = self
            .launch_tracker
            .observe(&self.compositor)
            .into_iter()
            .map(|launch| Task::future(sqlite_app_launches::record(launch)).discard())
            .collect();
        if config_reloaded {
            tasks.push(self.read_niri_workspace_order());
        }
        self.derive_window_snapshot();
        tasks.push(self.refocus_session());
        if focus_changed {
            self.widget_groups.collapse_when_auto_collapsing();
        }
        if windows_changed {
            tasks.push(
                self.favicons
                    .track_titles(&self.compositor)
                    .map(Message::FaviconsResolved),
            );
        }
        if workspaces_changed {
            tasks.push(
                self.widget_groups
                    .sidebar_settings_group
                    .observe_workspace_switch(&self.window_snapshot)
                    .map(Message::SidebarSettings),
            );
        }
        if windows_changed || workspaces_changed || focus_changed {
            tasks.push(self.refresh_app_launcher());
        }
        if self.notifications.live_count() > 0 {
            tasks.push(self.reflow_toasts());
        }
        Task::batch(tasks)
    }

    pub(super) fn refresh_app_launcher(&mut self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.arc() else {
            return Task::none();
        };
        let snapshot = &self.window_snapshot;
        let now = self.clock.utc();
        let launchable_ranked = |scope: LaunchScope<'_>| -> Vec<LaunchableApp> {
            let mut listed_app_ids = HashSet::new();
            self.launch_tracker
                .rank(scope, now)
                .iter()
                .filter_map(|app_id| entries.launchable_app(app_id))
                .filter(|app| listed_app_ids.insert(app.app_id.clone()))
                .collect()
        };
        let frequent_by_output = snapshot
            .active_workspaces()
            .map(|active| {
                let (scope, launch_scope) = match active.name {
                    Some(workspace) => (
                        FrequentAppScope::ThisWorkspace,
                        LaunchScope::NamedWorkspace(workspace),
                    ),
                    None => (FrequentAppScope::AllWorkspaces, LaunchScope::EveryWorkspace),
                };
                let open_app_ids: HashSet<String> = snapshot
                    .open_app_ids(active.output)
                    .iter()
                    .filter_map(|app_id| entries.launchable_app(app_id))
                    .map(|app| app.app_id)
                    .collect();
                let apps = launchable_ranked(launch_scope)
                    .into_iter()
                    .map(|app| FrequentApp {
                        window_presence: if open_app_ids.contains(&app.app_id) {
                            WorkspaceWindowPresence::Open
                        } else {
                            WorkspaceWindowPresence::Absent
                        },
                        app,
                    })
                    .collect();
                (active.output.to_owned(), OutputFrequentApps { scope, apps })
            })
            .collect();
        let usage = AppLaunchUsage {
            frequent_by_output,
            most_used_app_ids: launchable_ranked(LaunchScope::EveryWorkspace)
                .into_iter()
                .map(|app| app.app_id)
                .collect(),
            open_windows: open_windows_by_app(self.compositor.windows.values(), &entries),
        };
        self.widget_groups.app_launcher_group.adopt_usage(usage);
        self.resolve_icons()
    }

    pub(super) fn resolve_icons(&mut self) -> Task<Message> {
        let app_keys = desktop_entry_app_icons::app_icon_keys(&self.window_snapshot);
        let theme_keys = desktop_entry_app_icons::theme_icon_keys(&self.window_snapshot);
        self.desktop_entries
            .resolve_missing(app_keys, theme_keys)
            .map(Message::IconsResolved)
    }
}

pub(super) fn report_missing_compositor() -> Task<Message> {
    if CompositorKind::detected().is_some() {
        return Task::none();
    }
    Task::done(Message::CompositorRequestFailed(
        CompositorRequestFailure::Unreachable {
            request: "follow windows and workspaces".to_owned(),
            reason: missing_compositor_reason(),
        },
    ))
}

const UNNAMED_FULLSCREEN_WINDOW: &str = "A window without an app ID";

fn output_coverage(window: &CompositorWindow, output: &OutputInfo) -> OutputCoverage {
    let (window_width, window_height) = window.layout.window_size;
    let fills_output = i64::from(window_width) == i64::from(output.logical_width)
        && i64::from(window_height) == i64::from(output.logical_height);
    let half_output_width = f64::from(output.logical_width) / 2.0;
    let in_view = window
        .layout
        .tile_position_in_view
        .is_none_or(|(tile_x, _)| tile_x.abs() < half_output_width);
    if fills_output && in_view {
        OutputCoverage::Filled
    } else {
        OutputCoverage::Partial
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputCoverage {
    Filled,
    Partial,
}

fn focused_workspace_fullscreen_window<'a>(
    compositor: &'a CompositorWindowState,
    outputs: &HashMap<String, OutputInfo>,
) -> Option<&'a CompositorWindow> {
    let workspace = compositor.focused_workspace()?;
    let output = outputs.get(workspace.output.as_deref()?)?;
    compositor.windows.values().find(|window| {
        window.workspace_id == Some(workspace.id)
            && output_coverage(window, output) == OutputCoverage::Filled
    })
}

pub(crate) fn gamepad_window_claim<'a>(
    compositor: &'a CompositorWindowState,
    outputs: &HashMap<String, OutputInfo>,
) -> GamepadWindowClaim<'a> {
    if let Some(window) = focused_workspace_fullscreen_window(compositor, outputs) {
        return GamepadWindowClaim::Fullscreen(
            window
                .app_id
                .as_deref()
                .unwrap_or(UNNAMED_FULLSCREEN_WINDOW),
        );
    }
    compositor
        .windows
        .values()
        .find(|window| window.is_focused)
        .and_then(|window| window.app_id.as_deref())
        .map_or(GamepadWindowClaim::Unclaimed, GamepadWindowClaim::Focused)
}

pub(super) fn switch_heat_effect(message: &Message) -> SwitchHeatEffect {
    match message {
        Message::Compositor(_)
        | Message::SessionFocus(_)
        | Message::NiriWindow(_)
        | Message::Clock(_) => SwitchHeatEffect::Reheats,
        _ => SwitchHeatEffect::Unaffected,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SwitchHeatEffect {
    Reheats,
    Unaffected,
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::desktop_windows_apps::{
        compositor_test_windows::{unnamed_compositor_workspace, unplaced_compositor_window},
        compositor_window_state::{CompositorWindowLayout, CompositorWorkspace},
    };

    use super::*;

    const GAME_WORKSPACE: u64 = 1;
    const OTHER_WORKSPACE: u64 = 2;
    const FULL_OUTPUT_SIZE: (i32, i32) = (2560, 1440);
    const MAXIMIZED_COLUMN_SIZE: (i32, i32) = (2409, 1348);

    fn output_of_size(logical_width: u32, logical_height: u32) -> OutputInfo {
        OutputInfo {
            make: None,
            model: None,
            serial: None,
            mode: None,
            scale: 1.5,
            logical_width,
            logical_height,
        }
    }

    fn outputs() -> HashMap<String, OutputInfo> {
        HashMap::from([
            ("DP-1".to_owned(), output_of_size(2560, 1440)),
            ("DP-2".to_owned(), output_of_size(1920, 1080)),
        ])
    }

    fn workspace_on(id: u64, output: &str, is_focused: bool) -> CompositorWorkspace {
        CompositorWorkspace {
            output: Some(output.to_owned()),
            is_focused,
            ..unnamed_compositor_workspace(id)
        }
    }

    fn window_sized(
        id: u64,
        app_id: &str,
        workspace_id: u64,
        window_size: (i32, i32),
        is_focused: bool,
    ) -> CompositorWindow {
        let unplaced = unplaced_compositor_window(id);
        CompositorWindow {
            app_id: Some(app_id.to_owned()),
            workspace_id: Some(workspace_id),
            is_focused,
            layout: CompositorWindowLayout {
                window_size,
                ..unplaced.layout
            },
            ..unplaced
        }
    }

    fn compositor(windows: Vec<CompositorWindow>) -> CompositorWindowState {
        CompositorWindowState::from_lists(
            [
                workspace_on(GAME_WORKSPACE, "DP-1", true),
                workspace_on(OTHER_WORKSPACE, "DP-2", false),
            ],
            windows,
        )
    }

    #[test]
    fn a_prompt_focused_above_a_fullscreen_game_leaves_the_game_claiming() {
        let compositor = compositor(vec![
            window_sized(
                1,
                "steam_app_275850",
                GAME_WORKSPACE,
                FULL_OUTPUT_SIZE,
                false,
            ),
            window_sized(2, "opensnitch-ui", GAME_WORKSPACE, (600, 400), true),
        ]);
        assert_eq!(
            gamepad_window_claim(&compositor, &outputs()),
            GamepadWindowClaim::Fullscreen("steam_app_275850")
        );
    }

    #[test]
    fn a_fullscreen_game_claims_while_no_window_has_focus() {
        let compositor = compositor(vec![window_sized(
            1,
            "slowroads",
            GAME_WORKSPACE,
            FULL_OUTPUT_SIZE,
            false,
        )]);
        assert_eq!(
            gamepad_window_claim(&compositor, &outputs()),
            GamepadWindowClaim::Fullscreen("slowroads")
        );
    }

    #[test]
    fn a_maximized_column_falls_back_to_the_focused_window() {
        let compositor = compositor(vec![window_sized(
            1,
            "firefox",
            GAME_WORKSPACE,
            MAXIMIZED_COLUMN_SIZE,
            true,
        )]);
        assert_eq!(
            gamepad_window_claim(&compositor, &outputs()),
            GamepadWindowClaim::Focused("firefox")
        );
    }

    #[test]
    fn a_fullscreen_window_on_another_output_does_not_claim() {
        let compositor = compositor(vec![
            window_sized(1, "steam_app_275850", OTHER_WORKSPACE, (1920, 1080), false),
            window_sized(2, "kitty", GAME_WORKSPACE, (600, 1348), true),
        ]);
        assert_eq!(
            gamepad_window_claim(&compositor, &outputs()),
            GamepadWindowClaim::Focused("kitty")
        );
    }

    #[test]
    fn a_fullscreen_window_scrolled_out_of_view_does_not_claim() {
        let mut scrolled_away = window_sized(
            1,
            "steam_app_275850",
            GAME_WORKSPACE,
            FULL_OUTPUT_SIZE,
            false,
        );
        scrolled_away.layout.tile_position_in_view = Some((-2560.0, 0.0));
        let compositor = compositor(vec![
            scrolled_away,
            window_sized(2, "firefox", GAME_WORKSPACE, (899, 1348), true),
        ]);
        assert_eq!(
            gamepad_window_claim(&compositor, &outputs()),
            GamepadWindowClaim::Focused("firefox")
        );
    }

    #[test]
    fn an_empty_desktop_is_unclaimed() {
        assert_eq!(
            gamepad_window_claim(&compositor(Vec::new()), &outputs()),
            GamepadWindowClaim::Unclaimed
        );
    }
}
