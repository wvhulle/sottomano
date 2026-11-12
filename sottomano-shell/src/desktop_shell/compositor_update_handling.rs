use std::collections::HashSet;

use iced::Task;
use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::CompositorRequestFailure;
use sottomano_desktop_sources::{
    compositor_window_ipc::{
        compositor_connection::{CompositorKind, WorkspaceConfigSource, missing_compositor_reason},
        compositor_window_state::{CompositorChange, CompositorUpdate},
    },
    desktop_windows_apps::{
        app_launch_history::LaunchScope,
        desktop_entry_app_catalog::{LaunchableApp, open_windows_by_app},
        desktop_entry_app_icons::{self},
        focused_window_menu_bar::FocusedWindowIdentity,
        workspace_window_snapshot,
    },
};
use sottomano_widgets::widget_groups::app_launcher_group::{
    AppLaunchUsage, FrequentApp, FrequentAppScope, OutputFrequentApps, WorkspaceWindowPresence,
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
        self.notifications
            .show_compositor_failure(
                failure,
                &self.widget_groups.toast_context(
                    &self.compositor,
                    self.desktop_entries.get(),
                    &self.config,
                    self.clock,
                ),
            )
            .map(Message::from)
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
        self.widget_groups
            .global_menu_group
            .observe_focused_window(FocusedWindowIdentity::from(
                self.compositor.focused_window(),
            ));
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
        if !self.notifications.live.is_empty() {
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

pub(super) fn switch_heat_effect(message: &Message) -> SwitchHeatEffect {
    match message {
        Message::Compositor(_)
        | Message::SessionFocus(_)
        | Message::CompositorStyling(_)
        | Message::Clock(_) => SwitchHeatEffect::Reheats,
        _ => SwitchHeatEffect::Unaffected,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SwitchHeatEffect {
    Reheats,
    Unaffected,
}
