use std::{collections::HashSet, sync::Arc};

use iced::Task;
use sottomano_compositor_model::{
    compositor_requests::CompositorRequestFailure,
    compositor_window_facts::WorkspaceName,
    compositor_window_state::{CompositorChange, CompositorUpdate},
};
use sottomano_compositors::compositor_connection::{
    CompositorKind, configured_workspace_names, missing_compositor_reason,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        app_launch_history::LaunchScope,
        configured_workspace_order::ConfiguredWorkspaces,
        desktop_entry_app_catalog::{LaunchableApp, open_windows_by_app},
        focused_window_menu_bar::FocusedWindowIdentity,
        workspace_window_snapshot,
    },
    theme_icons::icon_requests::{app_icon_request, window_icon_requests},
};
use sottomano_widgets::{
    cascading_menus::app_launcher_menu::app_ranking::{
        AppLaunchUsage, FrequentApp, FrequentAppScope, OutputFrequentApps, WorkspaceWindowPresence,
    },
    group_indicators::indicator_group_interaction::IndicatorGroupInteraction,
};

use crate::{
    desktop_shell::DesktopShell,
    shell_message::{Message, ShellInteraction},
    sqlite_app_launches,
};

impl DesktopShell {
    pub(crate) fn derive_window_snapshot(&mut self) {
        self.window_snapshot = workspace_window_snapshot::Snapshot::derive(
            &self.compositor,
            self.indicator_groups.output_windows.window_processes(),
        );
    }

    pub(crate) fn show_compositor_failure(
        &mut self,
        failure: &CompositorRequestFailure,
    ) -> Task<Message> {
        tracing::warn!("{failure}");
        self.derive_window_snapshot();
        self.notifications
            .show_compositor_failure(
                failure,
                &self
                    .indicator_groups
                    .toast_context(&self.compositor, &self.config, self.clock),
            )
            .map(Message::from)
    }

    pub(crate) fn read_configured_workspace_order(&self) -> Task<Message> {
        let config_home = self.config_home.clone();
        Task::future(async move { configured_workspace_names(&config_home).await }).and_then(
            |configured_names| {
                Task::done(Message::ConfiguredWorkspacesRead(
                    configured_names.map(ConfiguredWorkspaces::from_iter),
                ))
            },
        )
    }

    pub(crate) fn apply_compositor_update(&mut self, update: CompositorUpdate) -> Task<Message> {
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
            tasks.push(self.read_configured_workspace_order());
        }
        self.derive_window_snapshot();
        self.indicator_groups
            .output_windows
            .observe_active_workspaces(
                &self.window_snapshot,
                self.clock,
                self.config.output_windows.workspace_slide_duration,
            );
        tasks.push(self.refocus_session());
        if focus_changed {
            self.indicator_groups.collapse_when_auto_collapsing();
        }
        self.indicator_groups
            .window_menu_bar
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
                self.indicator_groups
                    .panel_menus
                    .appearance
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

    pub(crate) fn refresh_app_launcher(&mut self) -> Task<Message> {
        let entries = Arc::clone(&self.desktop_entries);
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
                    WorkspaceName::Named(workspace) => (
                        FrequentAppScope::ThisWorkspace,
                        LaunchScope::NamedWorkspace(workspace),
                    ),
                    WorkspaceName::Unnamed => {
                        (FrequentAppScope::AllWorkspaces, LaunchScope::EveryWorkspace)
                    }
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
        self.indicator_groups.app_launcher_menu.adopt_usage(usage);
        self.request_icons()
    }

    pub(crate) fn request_icons(&mut self) -> Task<Message> {
        let launcher_app_ids = self.indicator_groups.app_launcher_menu.catalog_app_ids();
        let requests = window_icon_requests(&self.window_snapshot)
            .into_iter()
            .chain(
                self.indicator_groups
                    .panel_menus
                    .session
                    .tray_group
                    .icon_requests(&self.config),
            )
            .chain(
                self.indicator_groups
                    .window_menu_bar
                    .icon_requests(&self.config),
            )
            .chain(
                launcher_app_ids
                    .iter()
                    .map(|app_id| app_icon_request(app_id)),
            )
            .chain(self.notifications.icon_requests())
            .chain(
                self.indicator_groups
                    .panel_menus
                    .gaming
                    .steam_games_group
                    .icon_requests(),
            );
        self.icon_catalog
            .request(requests, &self.desktop_entries)
            .map(Message::IconCatalog)
    }
}

pub(crate) fn report_missing_compositor() -> Task<Message> {
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

pub(crate) fn switch_heat_effect(message: &Message) -> SwitchHeatEffect {
    match message {
        Message::Compositor(_)
        | Message::SessionFocus(_)
        | Message::CompositorStyling(_)
        | Message::Clock(_)
        | Message::Interaction(ShellInteraction::Group(
            IndicatorGroupInteraction::SessionFocus(_)
            | IndicatorGroupInteraction::CompositorStyling(_)
            | IndicatorGroupInteraction::Clock(_),
        )) => SwitchHeatEffect::Reheats,
        _ => SwitchHeatEffect::Unaffected,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwitchHeatEffect {
    Reheats,
    Unaffected,
}
