use iced::Task;
use sottomano_desktop_sources::{
    desktop_windows_apps::desktop_entry_changes,
    system_protocol_clients::dbus_desktop_buses::DesktopBusEvent,
};
use sottomano_widgets::{
    hover_cards::hover_card_transition::CardRetention,
    indicator_groups::idle_pause_groups::IdlePauseMessage,
    notification_toasts::toast_updates::StoppedChangeWatch,
};

use crate::{
    desktop_shell::DesktopShell, monitors::monitor_surfaces::SurfaceRole, pointer_drag::ShellDrag,
    shell_message::Message,
};

impl DesktopShell {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn dispatch_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Output(change) => self.apply_output_change(change),
            Message::WindowGroup(msg) => self.update_window_group(msg),
            Message::AppLauncher(launcher_message) => self.update_app_launcher(launcher_message),
            Message::TrackerLoaded(launches) => self.load_launch_tracker(launches),
            Message::DesktopEntriesLoaded(entries) => self.load_desktop_entries(entries),
            Message::ReloadDesktopEntries => {
                Task::perform(desktop_entry_changes::load(), Message::DesktopEntriesLoaded)
            }
            Message::IconThemeChanged(theme_name) => self
                .icon_catalog
                .reindex(theme_name)
                .map(Message::IconCatalog),
            Message::IconCatalog(catalog_message) => self
                .icon_catalog
                .update(catalog_message, &self.desktop_entries)
                .map(Message::IconCatalog),
            Message::FaviconsResolved(raw) => {
                self.favicons.merge(raw).map(Message::FaviconsResolved)
            }
            Message::FirefoxProfileChanged(Ok(())) => self
                .favicons
                .retry_unresolved()
                .map(Message::FaviconsResolved),
            Message::FirefoxProfileChanged(Err(watch_failure)) => {
                self.show_change_watch_failure(StoppedChangeWatch::FirefoxProfile, &watch_failure)
            }
            Message::Gamepad(msg) => self.update_gamepad(msg),
            Message::Notification(msg) => self.update_notification(msg),
            Message::NotificationServerFailed(server_failure) => {
                tracing::error!("notifications: server stopped, restarting: {server_failure}");
                Task::none()
            }
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::OverlaySettings(msg) => self.update_overlay_settings(msg),
            Message::NotificationGroup(msg) => self.update_notification_group(msg),
            Message::QuietSchedule(msg) => self.update_quiet_schedule(msg),
            Message::Session(msg) => self.update_session(msg),
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::WindowMenuBar(msg) => self.update_window_menu_bar(msg),
            Message::VirtualMachines(msg) => self.update_virtual_machines(msg),
            Message::IdleTimeouts(IdlePauseMessage::ScreenPowerChangedByIdleAction(edge)) => self
                .indicator_groups
                .session_focus_group
                .screen_power_changed(
                    edge,
                    &self.config.indicator_groups.session_focus,
                    self.clock,
                )
                .map(Message::SessionFocus),
            Message::IdleTimeouts(msg) => self
                .indicator_groups
                .idle_pause_groups
                .update(msg, self.clock)
                .map(Message::IdleTimeouts),
            Message::GroupMenuState(msg) => self.update_group_menus(msg),
            Message::Interaction(interaction) => self.interact(interaction),
            Message::SoundThemeLoaded(theme) => {
                self.notifications.adopt_sound_theme(theme);
                Task::none()
            }
            Message::ConfigFileChanged(Ok(())) => self.reload_config(),
            Message::ConfigFileChanged(Err(watch_failure)) => {
                self.show_change_watch_failure(StoppedChangeWatch::ConfigFile, &watch_failure)
            }
            Message::ConfigLoaded(Ok(config)) => self.adopt_config(*config),
            Message::ConfigLoaded(Err(error)) => self.reject_config(&error),
            Message::Compositor(update) => self.apply_compositor_update(update),
            Message::WindowClosed(window) => self.forget_window(window),
            Message::PointerLeft => {
                self.pointer_drag = ShellDrag::Idle;
                self.leave_hover_card()
            }
            Message::SurfacePressed(surface) => {
                let on_toast = self
                    .surfaces
                    .get(&surface)
                    .is_some_and(|host| matches!(host.role, SurfaceRole::NotificationToasts));
                if on_toast
                    && self.hover_card.transition.shown_retention() == CardRetention::UntilDismissed
                {
                    self.dismiss_hover_card()
                } else {
                    Task::none()
                }
            }
            Message::PointerDrag(msg) => self.update_pointer_drag(msg),
            Message::CompositorRequestFailed(failure) => self.show_compositor_failure(&failure),
            Message::DesktopBus(Ok(DesktopBusEvent::Connected(buses))) => {
                self.desktop_buses = Some(buses);
                Task::none()
            }
            Message::DesktopBus(Ok(DesktopBusEvent::Disconnected)) => {
                tracing::warn!("desktop buses: connection closed, reconnecting");
                self.desktop_buses = None;
                Task::none()
            }
            Message::DesktopBus(Err(failure)) => {
                tracing::error!("desktop buses: {failure}");
                self.notifications
                    .show_bus_failure(
                        &failure,
                        &self.indicator_groups.toast_context(
                            &self.compositor,
                            &self.config,
                            self.clock,
                        ),
                    )
                    .map(Message::from)
            }
            Message::NiriConfigRead(Ok(configured)) => {
                self.configured_workspaces = Some(configured);
                Task::none()
            }
            Message::NiriConfigRead(Err(reason)) => {
                tracing::warn!("niri config: {reason}");
                Task::none()
            }
            Message::HoverCardHideElapsed => self.leave_hover_card_after_grace(),
            Message::TypingUnderCards(signal) => {
                self.typing_under_cards.update(
                    signal,
                    self.clock.instant,
                    &self.config.overlay_surfaces,
                );
                Task::none()
            }
            Message::OutputsResolved(outputs) => self.adopt_outputs(outputs),
            Message::Redraw => {
                self.notifications.prune(self.clock.instant);
                self.indicator_groups
                    .window_group
                    .settle_workspace_slides(self.clock.instant);
                Task::none()
            }
            group_message @ (Message::Weather(_)
            | Message::Wallpaper(_)
            | Message::Caldav(_)
            | Message::Power(_)
            | Message::Compute(_)
            | Message::MemoryBoard(_)
            | Message::Gpu(_)
            | Message::Storage(_)
            | Message::Drive(_)
            | Message::SidebarSettings(_)
            | Message::CompositorStyling(_)
            | Message::Clock(_)
            | Message::SystemdManager(_)
            | Message::JournalEvents(_)
            | Message::PendingUpgrades(_)
            | Message::WifiAdapters(_)
            | Message::MobileModems(_)
            | Message::SteamClient(_)
            | Message::EthernetPorts(_)
            | Message::BluetoothAdapters(_)
            | Message::Reachability(_)
            | Message::Tailnet(_)
            | Message::WireguardTunnels(_)
            | Message::NetworkManagerVpns(_)
            | Message::Monitor(_)
            | Message::ColorTemperature(_)
            | Message::VoiceDictation(_)
            | Message::Audio(_)
            | Message::ClaudeUsage(_)
            | Message::CopilotUsage(_)
            | Message::OpenRouterCredit(_)
            | Message::LlamaServer(_)
            | Message::PodmanEngines(_)
            | Message::NspawnContainers(_)
            | Message::CgroupMemoryLimits(_)
            | Message::DockerEngines(_)
            | Message::ApptainerContainers(_)
            | Message::VmwareMachines(_)
            | Message::VirtualBoxMachines(_)
            | Message::WaydroidContainer(_)
            | Message::FlatpakSandboxes(_)
            | Message::LxcContainers(_)
            | Message::IncusInstances(_)
            | Message::MicrovmGuests(_)
            | Message::CiRuns(_)
            | Message::SessionFocus(_)
            | Message::LoginSession(_)
            | Message::DarkMode(_)) => {
                self.indicator_groups
                    .route(group_message, &self.config, self.clock)
            }
            Message::HoverCardFadeElapsed
            | Message::LayoutChange { .. }
            | Message::BlurOptionChange { .. }
            | Message::PopUpReposition { .. }
            | Message::SetInputRegion { .. }
            | Message::RequestActivationToken { .. }
            | Message::LayerChange { .. }
            | Message::MarginChange { .. }
            | Message::ExclusiveZoneChange { .. }
            | Message::KeyboardInteractivityChange { .. }
            | Message::VirtualKeyboardPressed { .. }
            | Message::NewLayerShell { .. }
            | Message::NewBaseWindow { .. }
            | Message::NewPopUp { .. }
            | Message::NewMenu { .. }
            | Message::NewInputPanel { .. }
            | Message::RemoveWindow(_)
            | Message::RedrawWindow(_)
            | Message::SetForeignParent { .. }
            | Message::ExportToplevel { .. }
            | Message::UnexportToplevel(_)
            | Message::ForgetLastOutput => Task::none(),
        }
    }
}
