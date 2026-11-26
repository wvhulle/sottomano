use std::{collections::HashMap, sync::Arc};

use iced::Task;
use sottomano_compositor_model::{compositor_output_facts, compositor_requests::CompositorRequest};
use sottomano_desktop_sinks::desktop_windows_apps::status_notifier_tray_activation::TrayCommand;
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{
        app_launch_history::Launch, desktop_entries::DesktopEntries, window_edge_placement,
    },
    system_protocol_clients::shell_command_output::blocking::run,
};
use sottomano_widgets::{
    background_work::{compositor_request, logged_failure_task},
    cascading_menus::{
        app_launcher_menu::{self, AppLauncherMessage},
        menu_node_entries::PressDismissal,
        window_menu_bar::{
            self, WindowMenuBarInteraction, WindowMenuBarMessage, WindowMenuBarRequest,
        },
    },
    layer_surfaces::{ActivationTokenOutcome, activation_token_request},
    output_windows::{self, OutputWindowsInteraction, OutputWindowsMessage, window_list},
    panel_menus::{
        appearance_menu::{
            overlay_settings_group::{self, OverlaySettingsMessage},
            panel_settings_group::{self, PanelSettingsMessage},
        },
        gaming_menu::gamepad_group::{GamepadInteraction, GamepadMessage},
        media_menu::media_group::{self, MediaInteraction, MediaMessage},
        session_menu::tray_group::{self, TrayMessage},
        session_power_menu::session_power_group::{self, SessionMessage},
        virtualization_menu::virtual_machines_group::{
            self, VirtualMachinesInteraction, VirtualMachinesMessage,
        },
    },
    widgets::app_window_focus::AppWindowFocus,
};

use crate::{
    desktop_shell::{DesktopBusLink, DesktopShell},
    shell_message::Message,
};

impl DesktopShell {
    pub(crate) fn open_url(&mut self, url: String) -> Task<Message> {
        Task::batch([
            self.dismiss_hover_card(),
            Task::future(blocking("open url", move || {
                if let Err(open_failure) = run("xdg-open", &[&url]) {
                    tracing::warn!("opening {url}: {open_failure}");
                }
            }))
            .discard(),
        ])
    }

    pub(crate) fn update_overlay_settings(&mut self, msg: OverlaySettingsMessage) -> Task<Message> {
        match self.indicator_groups.overlay_settings_group.update(msg) {
            overlay_settings_group::Action::Run(task) => task.map(Message::OverlaySettings),
            overlay_settings_group::Action::ReflowToasts(task) => {
                Task::batch([self.reflow_toasts(), task.map(Message::OverlaySettings)])
            }
        }
    }

    pub(crate) fn load_launch_tracker(&mut self, launches: Vec<Launch>) -> Task<Message> {
        self.launch_tracker.load_from(launches);
        self.refresh_app_launcher()
    }

    pub(crate) fn load_desktop_entries(&mut self, entries: Arc<DesktopEntries>) -> Task<Message> {
        self.indicator_groups
            .app_launcher_menu
            .adopt_catalog(entries.launchable_apps());
        self.desktop_entries = entries;
        let reindexed_icons = self.icon_catalog.refresh().map(Message::IconCatalog);
        let requested_icons = self.request_icons();
        Task::batch([
            self.refresh_app_launcher(),
            reindexed_icons,
            requested_icons,
        ])
    }

    pub(crate) fn update_app_launcher(
        &mut self,
        launcher_message: AppLauncherMessage,
    ) -> Task<Message> {
        match self
            .indicator_groups
            .app_launcher_menu
            .update(launcher_message, &self.config)
        {
            app_launcher_menu::Action::Run(task) => task.map(Message::AppLauncher),
            app_launcher_menu::Action::RunAndDismissMenu(task) => {
                Task::batch([self.dismiss_hover_card(), task.map(Message::AppLauncher)])
            }
        }
    }

    pub(crate) fn adopt_outputs(
        &mut self,
        outputs: HashMap<String, compositor_output_facts::OutputInfo>,
    ) -> Task<Message> {
        self.outputs = outputs;
        self.reflow_toasts()
    }

    pub(crate) fn reflow_toasts(&mut self) -> Task<Message> {
        let placement = window_edge_placement::Placement::derive(&self.compositor);
        self.notifications
            .on_compositor_changed(
                &placement,
                &self.config,
                self.indicator_groups.notification_settings(),
                self.clock.instant,
            )
            .map(Message::from)
    }

    pub(crate) fn refocus_session(&mut self) -> Task<Message> {
        self.indicator_groups
            .session_focus_group
            .refocus(
                &self.compositor,
                self.indicator_groups.output_windows.window_processes(),
            )
            .map(Message::SessionFocus)
    }

    pub(crate) fn update_output_windows(
        &mut self,
        window_message: OutputWindowsMessage,
    ) -> Task<Message> {
        let action = self
            .indicator_groups
            .output_windows
            .update(window_message, self.clock);
        self.run_output_windows_action(action)
    }

    pub(crate) fn interact_output_windows(
        &mut self,
        window_interaction: OutputWindowsInteraction,
    ) -> Task<Message> {
        let action = self
            .indicator_groups
            .output_windows
            .interact(window_interaction);
        self.run_output_windows_action(action)
    }

    fn run_output_windows_action(&mut self, action: output_windows::Action) -> Task<Message> {
        let handled = match action {
            output_windows::Action::WindowList(window_list::Action::ToggleAudio(pid)) => self
                .indicator_groups
                .audio_group
                .toggle_window_mute(pid)
                .map(Message::Audio),
            output_windows::Action::WindowList(window_list::Action::None) => Task::none(),
            output_windows::Action::WindowList(window_list::Action::WindowProcessesPaired) => {
                self.derive_window_snapshot();
                self.request_icons()
            }
            output_windows::Action::WindowList(window_list::Action::Drag(drag)) => {
                self.update_pointer_drag(drag)
            }
        };
        Task::batch([handled, self.refocus_session()])
    }

    pub(crate) fn update_gamepad(&mut self, msg: GamepadMessage) -> Task<Message> {
        self.indicator_groups
            .gamepad_group
            .update(msg, &self.config.indicator_groups.gamepad)
            .into_task()
    }

    pub(crate) fn interact_gamepad(&mut self, interaction: GamepadInteraction) -> Task<Message> {
        self.indicator_groups
            .gamepad_group
            .interact(interaction)
            .into_task()
    }

    pub(crate) fn update_media(&mut self, msg: MediaMessage) -> Task<Message> {
        let action = self.indicator_groups.media_group.update(msg, self.clock);
        self.run_media_action(action)
    }

    pub(crate) fn interact_media(&mut self, interaction: MediaInteraction) -> Task<Message> {
        let action = self
            .indicator_groups
            .media_group
            .interact(interaction, self.clock);
        self.run_media_action(action)
    }

    fn run_media_action(&self, action: media_group::Action) -> Task<Message> {
        match action {
            media_group::Action::Run(task) => task.map(Message::Media),
            media_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Media)
            }
        }
    }

    pub(crate) fn update_session(&mut self, msg: SessionMessage) -> Task<Message> {
        match self.indicator_groups.session_power_group.update(msg) {
            session_power_group::Action::Run(task) => task.map(Message::Session),
            session_power_group::Action::Performed(task) => {
                Task::batch([self.dismiss_hover_card(), task.map(Message::Session)])
            }
        }
    }

    pub(crate) fn update_tray(&mut self, msg: TrayMessage) -> Task<Message> {
        match self.indicator_groups.tray_group.update(msg, self.clock) {
            tray_group::Action::None => Task::none(),
            tray_group::Action::Run(task) => task.map(Message::Tray),
            tray_group::Action::SendWithActivationToken(command) => {
                self.send_tray_command_with_activation_token(command)
            }
            tray_group::Action::MenuItemActivated { command, dismissal } => {
                let menu_click = self.send_tray_command_with_activation_token(command);
                match dismissal {
                    PressDismissal::ClosesMenu => {
                        Task::batch([self.dismiss_hover_card(), menu_click])
                    }
                    PressDismissal::KeepsMenuOpen => menu_click,
                }
            }
            tray_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Tray)
            }
        }
    }

    pub(crate) fn update_window_menu_bar(&mut self, msg: WindowMenuBarMessage) -> Task<Message> {
        let action = self.indicator_groups.window_menu_bar.update(msg);
        self.run_window_menu_bar_action(action)
    }

    pub(crate) fn interact_window_menu_bar(
        &mut self,
        interaction: WindowMenuBarInteraction,
    ) -> Task<Message> {
        let action = self.indicator_groups.window_menu_bar.interact(interaction);
        self.run_window_menu_bar_action(action)
    }

    fn run_window_menu_bar_action(&mut self, action: window_menu_bar::Action) -> Task<Message> {
        match action {
            window_menu_bar::Action::None => Task::none(),
            window_menu_bar::Action::Run(task) => task.map(Message::WindowMenuBar),
            window_menu_bar::Action::Activate { request, dismissal } => {
                let activation = self.send_window_menu_bar_request(request);
                match dismissal {
                    PressDismissal::ClosesMenu => {
                        Task::batch([self.dismiss_hover_card(), activation])
                    }
                    PressDismissal::KeepsMenuOpen => activation,
                }
            }
        }
    }

    fn send_window_menu_bar_request(&self, request: WindowMenuBarRequest) -> Task<Message> {
        let DesktopBusLink::Connected(buses) = &self.desktop_buses else {
            return Task::none();
        };
        logged_failure_task(
            format!("window menu bar: {request:?}"),
            request.send(buses.session.clone()),
        )
    }

    fn send_tray_command_with_activation_token(&self, command: TrayCommand) -> Task<Message> {
        let hovered_output = self.hovered_output();
        let Some((panel_surface, _)) = self
            .panels()
            .find(|(_, panel_output)| *panel_output == hovered_output)
        else {
            return Task::done(Message::Tray(TrayMessage::Send {
                command,
                token: ActivationTokenOutcome::Unavailable,
            }));
        };
        let token_request = activation_token_request(panel_surface);
        Task::batch([
            Task::done(Message::from(token_request.surface_request)),
            Task::future(token_request.token_arrival).map(move |token| {
                Message::Tray(TrayMessage::Send {
                    command: command.clone(),
                    token,
                })
            }),
        ])
    }

    pub(crate) fn update_virtual_machines(&mut self, msg: VirtualMachinesMessage) -> Task<Message> {
        let action = self
            .indicator_groups
            .virtual_machines_group
            .update(msg, self.clock);
        self.run_virtual_machines_action(action)
    }

    pub(crate) fn interact_virtual_machines(
        &mut self,
        interaction: VirtualMachinesInteraction,
    ) -> Task<Message> {
        let action = self
            .indicator_groups
            .virtual_machines_group
            .interact(interaction);
        self.run_virtual_machines_action(action)
    }

    fn run_virtual_machines_action(&self, action: virtual_machines_group::Action) -> Task<Message> {
        match action {
            virtual_machines_group::Action::Run(task) => task.map(Message::VirtualMachines),
            virtual_machines_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::VirtualMachines)
            }
        }
    }

    pub(crate) fn focus_app_window<M: Send + 'static>(
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

    pub(crate) fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match self.indicator_groups.panel_settings_group.update(msg) {
            panel_settings_group::Action::Run(task) => task.map(Message::PanelSettings),
            panel_settings_group::Action::Reflow(task) => {
                Task::batch([self.reserve(), task.map(Message::PanelSettings)])
            }
        }
    }
}
