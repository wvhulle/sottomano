use iced::Task;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use sottomano_sources::desktop_windows_apps::{
    desktop_entry_app_icons::{self},
    niri_action_requests, niri_window_snapshot,
};

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

pub(crate) fn focused_app_id(niri: &EventStreamState) -> Option<&str> {
    niri.windows
        .windows
        .values()
        .find(|window| window.is_focused)
        .and_then(|window| window.app_id.as_deref())
}

pub(super) fn reheats_switch_heat(message: &Message) -> bool {
    matches!(
        message,
        Message::Niri(_) | Message::SessionFocus(_) | Message::NiriWindow(_) | Message::Clock(_)
    )
}
