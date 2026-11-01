use std::collections::HashMap;

use iced::Task;
use niri_ipc::{
    Window,
    state::{EventStreamState, EventStreamStatePart},
};
use sottomano_sources::desktop_windows_apps::{
    desktop_entry_app_icons::{self},
    niri_action_requests,
    niri_output_facts::OutputInfo,
    niri_window_snapshot,
};
use sottomano_widgets::widget_groups::gamepad_group::GamepadWindowClaim;

use crate::{
    desktop_shell::{DesktopShell, sqlite_app_launches},
    shell_message::Message,
};

impl DesktopShell {
    pub(super) fn derive_niri_snapshot(&mut self) {
        self.niri_snapshot = niri_window_snapshot::Snapshot::derive(
            &self.niri,
            self.widget_groups.window_group.window_processes(),
        );
    }

    pub(super) fn show_niri_failure(
        &mut self,
        failure: &niri_action_requests::ActionFailure,
    ) -> Task<Message> {
        tracing::warn!("{failure}");
        self.derive_niri_snapshot();
        self.notifications.show_niri_failure(
            failure,
            &self.widget_groups.toast_context(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.clock,
            ),
        )
    }

    pub(super) fn apply_niri_events(&mut self, events: Vec<niri_ipc::Event>) -> Task<Message> {
        let mut window_change = false;
        let mut workspace_change = false;
        let mut focus_change = false;
        let mut tasks = Vec::new();
        for event in events {
            window_change |= matches!(
                event,
                niri_ipc::Event::WindowsChanged { .. }
                    | niri_ipc::Event::WindowOpenedOrChanged { .. }
                    | niri_ipc::Event::WindowClosed { .. }
            );
            workspace_change |= matches!(
                event,
                niri_ipc::Event::WorkspacesChanged { .. }
                    | niri_ipc::Event::WorkspaceActivated { .. }
            );
            focus_change |= matches!(event, niri_ipc::Event::WindowFocusChanged { .. });
            if matches!(event, niri_ipc::Event::ConfigLoaded { failed: false }) {
                tasks.push(Task::perform(
                    self.niri_config.read_workspaces(),
                    Message::NiriConfigRead,
                ));
            }
            if let Some(launch) = self.launch_tracker.observe(&event) {
                tasks.push(Task::future(sqlite_app_launches::record(launch)).discard());
            }
            let _ = self.niri.apply(event);
        }
        self.derive_niri_snapshot();
        tasks.push(self.refocus_session());
        if focus_change {
            self.widget_groups.collapse_when_auto_collapsing();
        }
        if window_change {
            tasks.push(
                self.favicons
                    .track_titles(&self.niri)
                    .map(Message::FaviconsResolved),
            );
        }
        if workspace_change {
            tasks.push(
                self.widget_groups
                    .sidebar_settings_group
                    .observe_workspace_switch(&self.niri_snapshot)
                    .map(Message::SidebarSettings),
            );
        }
        if window_change || workspace_change {
            tasks.push(self.refresh_quick_launch());
        }
        if self.notifications.is_active() {
            tasks.push(self.reflow_toasts());
        }
        Task::batch(tasks)
    }

    pub(super) fn refresh_quick_launch(&mut self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.arc() else {
            return Task::none();
        };
        let snapshot = &self.niri_snapshot;
        let now = self.clock.utc();
        let ranked = snapshot
            .active_workspaces()
            .filter_map(|active| {
                let workspace = active.name?;
                let open = snapshot.open_app_ids(active.output);
                let apps = self.launch_tracker.rank(workspace, &open, now, |app_id| {
                    entries.is_launchable(app_id)
                });
                (!apps.is_empty()).then(|| (active.output.to_owned(), apps))
            })
            .collect();
        self.widget_groups.window_group.set_ranked(ranked);
        self.resolve_icons()
    }

    pub(super) fn resolve_icons(&mut self) -> Task<Message> {
        let mut app_keys = desktop_entry_app_icons::app_icon_keys(&self.niri_snapshot);
        app_keys.extend(self.widget_groups.window_group.icon_keys());
        let theme_keys = desktop_entry_app_icons::theme_icon_keys(&self.niri_snapshot);
        self.desktop_entries
            .resolve_missing(app_keys, theme_keys)
            .map(Message::IconsResolved)
    }
}

const UNNAMED_FULLSCREEN_WINDOW: &str = "A window without an app ID";

fn fills_output_in_view(window: &Window, output: &OutputInfo) -> bool {
    let (window_width, window_height) = window.layout.window_size;
    let fills_output = i64::from(window_width) == i64::from(output.logical_width)
        && i64::from(window_height) == i64::from(output.logical_height);
    let half_output_width = f64::from(output.logical_width) / 2.0;
    fills_output
        && window
            .layout
            .tile_pos_in_workspace_view
            .is_none_or(|(tile_x, _)| tile_x.abs() < half_output_width)
}

fn focused_workspace_fullscreen_window<'a>(
    niri: &'a EventStreamState,
    outputs: &HashMap<String, OutputInfo>,
) -> Option<&'a Window> {
    let workspace = niri
        .workspaces
        .workspaces
        .values()
        .find(|workspace| workspace.is_focused)?;
    let output = outputs.get(workspace.output.as_deref()?)?;
    niri.windows.windows.values().find(|window| {
        window.workspace_id == Some(workspace.id) && fills_output_in_view(window, output)
    })
}

pub(crate) fn gamepad_window_claim<'a>(
    niri: &'a EventStreamState,
    outputs: &HashMap<String, OutputInfo>,
) -> GamepadWindowClaim<'a> {
    if let Some(window) = focused_workspace_fullscreen_window(niri, outputs) {
        return GamepadWindowClaim::Fullscreen(
            window
                .app_id
                .as_deref()
                .unwrap_or(UNNAMED_FULLSCREEN_WINDOW),
        );
    }
    niri.windows
        .windows
        .values()
        .find(|window| window.is_focused)
        .and_then(|window| window.app_id.as_deref())
        .map_or(GamepadWindowClaim::Unclaimed, GamepadWindowClaim::Focused)
}

pub(super) fn reheats_switch_heat(message: &Message) -> bool {
    matches!(
        message,
        Message::Niri(_) | Message::SessionFocus(_) | Message::NiriWindow(_) | Message::Clock(_)
    )
}

#[cfg(test)]
mod tests {
    use niri_ipc::{WindowLayout, Workspace};

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

    fn workspace_on(id: u64, output: &str, is_focused: bool) -> Workspace {
        Workspace {
            id,
            idx: 1,
            name: None,
            output: Some(output.to_owned()),
            is_urgent: false,
            is_active: true,
            is_focused,
            active_window_id: None,
        }
    }

    fn window_sized(
        id: u64,
        app_id: &str,
        workspace_id: u64,
        window_size: (i32, i32),
        is_focused: bool,
    ) -> Window {
        Window {
            id,
            title: None,
            app_id: Some(app_id.to_owned()),
            pid: None,
            workspace_id: Some(workspace_id),
            is_focused,
            is_floating: false,
            is_urgent: false,
            layout: WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size,
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    fn niri_state(windows: Vec<Window>) -> EventStreamState {
        let mut niri = EventStreamState::default();
        niri.workspaces.workspaces = [
            workspace_on(GAME_WORKSPACE, "DP-1", true),
            workspace_on(OTHER_WORKSPACE, "DP-2", false),
        ]
        .into_iter()
        .map(|workspace| (workspace.id, workspace))
        .collect();
        niri.windows.windows = windows
            .into_iter()
            .map(|window| (window.id, window))
            .collect();
        niri
    }

    #[test]
    fn a_prompt_focused_above_a_fullscreen_game_leaves_the_game_claiming() {
        let niri = niri_state(vec![
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
            gamepad_window_claim(&niri, &outputs()),
            GamepadWindowClaim::Fullscreen("steam_app_275850")
        );
    }

    #[test]
    fn a_fullscreen_game_claims_while_no_window_has_focus() {
        let niri = niri_state(vec![window_sized(
            1,
            "slowroads",
            GAME_WORKSPACE,
            FULL_OUTPUT_SIZE,
            false,
        )]);
        assert_eq!(
            gamepad_window_claim(&niri, &outputs()),
            GamepadWindowClaim::Fullscreen("slowroads")
        );
    }

    #[test]
    fn a_maximized_column_falls_back_to_the_focused_window() {
        let niri = niri_state(vec![window_sized(
            1,
            "firefox",
            GAME_WORKSPACE,
            MAXIMIZED_COLUMN_SIZE,
            true,
        )]);
        assert_eq!(
            gamepad_window_claim(&niri, &outputs()),
            GamepadWindowClaim::Focused("firefox")
        );
    }

    #[test]
    fn a_fullscreen_window_on_another_output_does_not_claim() {
        let niri = niri_state(vec![
            window_sized(1, "steam_app_275850", OTHER_WORKSPACE, (1920, 1080), false),
            window_sized(2, "kitty", GAME_WORKSPACE, (600, 1348), true),
        ]);
        assert_eq!(
            gamepad_window_claim(&niri, &outputs()),
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
        scrolled_away.layout.tile_pos_in_workspace_view = Some((-2560.0, 0.0));
        let niri = niri_state(vec![
            scrolled_away,
            window_sized(2, "firefox", GAME_WORKSPACE, (899, 1348), true),
        ]);
        assert_eq!(
            gamepad_window_claim(&niri, &outputs()),
            GamepadWindowClaim::Focused("firefox")
        );
    }

    #[test]
    fn an_empty_desktop_is_unclaimed() {
        assert_eq!(
            gamepad_window_claim(&niri_state(Vec::new()), &outputs()),
            GamepadWindowClaim::Unclaimed
        );
    }
}
