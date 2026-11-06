use std::{collections::HashMap, sync::Arc};

use iced::Task;
use sottomano_desktop_sinks::desktop_windows_apps::compositor_requests::CompositorRequest;
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{
        app_launch_history::Launch, desktop_entry_app_icons::DesktopEntries, niri_output_facts,
        window_edge_placement,
    },
    system_protocol_clients::shell_command_output::blocking::run,
};
use sottomano_widgets::{
    background_work::compositor_request,
    widget_groups::{
        app_launcher_group::{self, AppLauncherMessage},
        gamepad_group::GamepadMessage,
        media_group::{self, MediaMessage},
        overlay_settings_group::{self, OverlaySettingsMessage},
        panel_settings_group::{self, PanelSettingsMessage},
        session_power_group::{self, SessionMessage},
        tray_group::{self, TrayMessage},
        virtual_machines_group::{self, VirtualMachinesMessage},
        window_group::{self, WindowGroupMessage, window_list},
    },
    widgets::app_window_focus::AppWindowFocus,
};

use crate::{
    desktop_shell::{DesktopShell, compositor_update_handling::gamepad_window_claim},
    shell_message::Message,
};

impl DesktopShell {
    pub(super) fn open_url(&mut self, url: String) -> Task<Message> {
        Task::batch([
            self.dismiss_hover_card(),
            Task::future(blocking("open url", move || run("xdg-open", &[&url]))).discard(),
        ])
    }

    pub(super) fn update_overlay_settings(&mut self, msg: OverlaySettingsMessage) -> Task<Message> {
        match self.widget_groups.overlay_settings_group.update(msg) {
            overlay_settings_group::Action::Run(task) => task.map(Message::OverlaySettings),
            overlay_settings_group::Action::ReflowToasts(task) => {
                Task::batch([self.reflow_toasts(), task.map(Message::OverlaySettings)])
            }
        }
    }

    pub(super) fn load_launch_tracker(&mut self, launches: Vec<Launch>) -> Task<Message> {
        self.launch_tracker.load_from(launches);
        self.refresh_app_launcher()
    }

    pub(super) fn load_desktop_entries(&mut self, entries: Arc<DesktopEntries>) -> Task<Message> {
        let launcher = &mut self.widget_groups.app_launcher_group;
        launcher.adopt_catalog(entries.launchable_apps());
        self.desktop_entries.set(entries);
        let catalog_icons = self
            .desktop_entries
            .resolve_catalog_icons(launcher.catalog_app_ids())
            .map(Message::IconsResolved);
        Task::batch([self.refresh_app_launcher(), catalog_icons])
    }

    pub(super) fn update_app_launcher(
        &mut self,
        launcher_message: AppLauncherMessage,
    ) -> Task<Message> {
        match self
            .widget_groups
            .app_launcher_group
            .update(launcher_message, &self.config)
        {
            app_launcher_group::Action::Run(task) => task.map(Message::AppLauncher),
            app_launcher_group::Action::RunAndDismissMenu(task) => {
                Task::batch([self.dismiss_hover_card(), task.map(Message::AppLauncher)])
            }
        }
    }

    pub(super) fn adopt_outputs(
        &mut self,
        outputs: HashMap<String, niri_output_facts::OutputInfo>,
    ) -> Task<Message> {
        self.outputs = outputs;
        self.reflow_toasts()
    }

    pub(super) fn reflow_toasts(&mut self) -> Task<Message> {
        let placement = window_edge_placement::Placement::derive(&self.compositor);
        self.notifications.on_niri_changed(
            &placement,
            &self.config,
            self.widget_groups.notification_settings(),
            self.clock.instant,
        )
    }

    pub(super) fn refocus_session(&mut self) -> Task<Message> {
        self.widget_groups
            .session_focus_group
            .refocus(
                &self.compositor,
                self.widget_groups.window_group.window_processes(),
            )
            .map(Message::SessionFocus)
    }

    pub(super) fn update_window_group(
        &mut self,
        window_message: WindowGroupMessage,
    ) -> Task<Message> {
        let handled = match self
            .widget_groups
            .window_group
            .update(window_message, self.clock)
        {
            window_group::Action::WindowList(window_list::Action::ToggleAudio(pid)) => self
                .widget_groups
                .audio_group
                .toggle_window_mute(pid)
                .map(Message::Audio),
            window_group::Action::WindowList(window_list::Action::None) => Task::none(),
            window_group::Action::WindowList(window_list::Action::WindowProcessesPaired) => {
                self.derive_window_snapshot();
                self.resolve_icons()
            }
            window_group::Action::WindowList(window_list::Action::Drag(drag)) => {
                self.update_pointer_drag(drag)
            }
        };
        Task::batch([handled, self.refocus_session()])
    }

    pub(super) fn update_gamepad(&mut self, msg: GamepadMessage) -> Task<Message> {
        self.widget_groups
            .gamepad_group
            .update(
                msg,
                &self.config.widget_groups.gamepad,
                gamepad_window_claim(&self.compositor, &self.outputs),
            )
            .into_task()
    }

    pub(super) fn update_media(&mut self, msg: MediaMessage) -> Task<Message> {
        match self.widget_groups.media_group.update(msg, self.clock) {
            media_group::Action::Run(task) => task.map(Message::Media),
            media_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Media)
            }
        }
    }

    pub(super) fn update_session(&mut self, msg: SessionMessage) -> Task<Message> {
        match self.widget_groups.session_power_group.update(msg) {
            session_power_group::Action::Run(task) => task.map(Message::Session),
            session_power_group::Action::Performed(task) => {
                Task::batch([self.dismiss_hover_card(), task.map(Message::Session)])
            }
        }
    }

    pub(super) fn update_tray(&mut self, msg: TrayMessage) -> Task<Message> {
        match self.widget_groups.tray_group.update(msg, self.clock) {
            tray_group::Action::None => Task::none(),
            tray_group::Action::Run(task) => task.map(Message::Tray),
            tray_group::Action::MenuItemActivated(task) => {
                Task::batch([self.dismiss_hover_card(), task.map(Message::Tray)])
            }
            tray_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Tray)
            }
        }
    }

    pub(super) fn update_virtual_machines(&mut self, msg: VirtualMachinesMessage) -> Task<Message> {
        match self
            .widget_groups
            .virtual_machines_group
            .update(msg, self.clock)
        {
            virtual_machines_group::Action::Run(task) => task.map(Message::VirtualMachines),
            virtual_machines_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::VirtualMachines)
            }
        }
    }

    pub(super) fn focus_app_window<M: Send + 'static>(
        &self,
        focus: AppWindowFocus<M>,
        lift: fn(M) -> Message,
    ) -> Task<Message> {
        let placement = window_edge_placement::Placement::derive(&self.compositor);
        let Some(window) = placement.window_owned_by(&focus.owner) else {
            tracing::debug!("no single window owned by {:?}", focus.owner);
            return Task::done(lift(focus.unmatched));
        };
        compositor_request(CompositorRequest::FocusWindow(window))
    }

    pub(super) fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match self.widget_groups.panel_settings_group.update(msg) {
            panel_settings_group::Action::Run(task) => task.map(Message::PanelSettings),
            panel_settings_group::Action::Reflow(task) => {
                Task::batch([self.reserve(), task.map(Message::PanelSettings)])
            }
        }
    }
}
