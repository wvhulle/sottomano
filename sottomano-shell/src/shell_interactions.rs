use iced::Task;
use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::CompositorRequest;
use sottomano_widgets::{
    background_work::{compositor_request, copy_to_clipboard},
    group_menus::group_menu_state::{GroupMenuMessage, PanelPin, PanelPinDuration},
    hover_cards::PopupMessage,
    indicator_groups::{
        appearance_presets_group::AppearancePresetsMessage,
        indicator_group_interaction::{IndicatorGroupBusRequest, IndicatorGroupInteraction},
        sidebar_settings_group::SidebarSettingsMessage,
        workspaces_group::WorkspacesMessage,
    },
};

use crate::{
    desktop_shell::DesktopShell,
    shell_message::{Message, ShellInteraction},
};

impl DesktopShell {
    pub(crate) fn interact(&mut self, interaction: ShellInteraction) -> Task<Message> {
        match interaction {
            ShellInteraction::Group(group_interaction) => self.interact_group(group_interaction),
            ShellInteraction::Popup(popup_message) => self.update_popup(popup_message),
            ShellInteraction::Notification(notification) => self.update_notification(notification),
            ShellInteraction::DismissPressed => {
                self.indicator_groups.collapse_when_auto_collapsing();
                Task::none()
            }
            ShellInteraction::SidebarWidthChanged { window, width } => {
                self.update_sidebar_width(window, width)
            }
            ShellInteraction::Workspaces(workspaces_message) => {
                self.update_workspaces(workspaces_message)
            }
            ShellInteraction::SurfaceRequest(surface_request) => {
                Task::done(Message::from(surface_request))
            }
            ShellInteraction::CompositorRequestFailed(failure) => {
                self.show_compositor_failure(&failure)
            }
        }
    }

    fn update_popup(&mut self, popup_message: PopupMessage) -> Task<Message> {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => self.update_hover_card(hover_card),
            PopupMessage::CopyText(text) => copy_to_clipboard(text),
            PopupMessage::OpenUrl(url) => self.open_url(url),
            PopupMessage::FocusWindow(window) => Task::batch([
                self.dismiss_hover_card(),
                compositor_request(CompositorRequest::FocusWindow(window)),
            ]),
            PopupMessage::RefreshHoveredIndicator => self.refresh_hovered_indicator(),
            PopupMessage::WindowGroup(window_interaction) => {
                self.interact_window_group(window_interaction)
            }
            PopupMessage::ReleaseFromPanel(panel_pin) => self.release_from_panel(panel_pin),
        }
    }

    fn release_from_panel(&mut self, panel_pin: PanelPin) -> Task<Message> {
        let PanelPin {
            menu,
            entry,
            duration,
        } = panel_pin;
        let release = match duration {
            PanelPinDuration::Permanent => GroupMenuMessage::TogglePin { menu, entry },
            PanelPinDuration::UntilAlertEnds => GroupMenuMessage::DismissAlertPin { menu, entry },
        };
        Task::batch([self.dismiss_hover_card(), self.update_group_menus(release)])
    }

    fn update_workspaces(&mut self, workspaces_message: WorkspacesMessage) -> Task<Message> {
        match workspaces_message {
            WorkspacesMessage::HoverCard(hover_card) => self.update_hover_card(hover_card),
            WorkspacesMessage::Drag(drag) => self.update_pointer_drag(drag),
            WorkspacesMessage::RestoreOrder { output } => self.restore_workspace_order(&output),
            WorkspacesMessage::ToggleEmptyWorkspaces => {
                self.interact_shell_group(IndicatorGroupInteraction::SidebarSettings(
                    SidebarSettingsMessage::ToggleEmptyWorkspaces,
                ))
            }
            WorkspacesMessage::Focus(workspace_id) => {
                compositor_request(CompositorRequest::FocusWorkspace(workspace_id))
            }
        }
    }

    fn interact_group(&mut self, group_interaction: IndicatorGroupInteraction) -> Task<Message> {
        match IndicatorGroupBusRequest::try_from(group_interaction) {
            Ok(bus_request) => {
                let Some(buses) = &self.desktop_buses else {
                    return Task::none();
                };
                self.indicator_groups
                    .request_on_buses(bus_request, &self.config, buses, self.clock)
            }
            Err(local_interaction) => self.interact_shell_group(local_interaction),
        }
    }

    fn interact_shell_group(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
    ) -> Task<Message> {
        match group_interaction {
            IndicatorGroupInteraction::WindowGroup(interaction) => {
                self.interact_window_group(interaction)
            }
            IndicatorGroupInteraction::AppLauncher(message) => self.update_app_launcher(message),
            IndicatorGroupInteraction::Gamepad(interaction) => self.interact_gamepad(interaction),
            IndicatorGroupInteraction::PanelSettings(message) => {
                self.update_panel_settings(message)
            }
            IndicatorGroupInteraction::OverlaySettings(message) => {
                self.update_overlay_settings(message)
            }
            IndicatorGroupInteraction::NotificationGroup(message) => {
                self.update_notification_group(message)
            }
            IndicatorGroupInteraction::QuietSchedule(message) => {
                self.update_quiet_schedule(message)
            }
            IndicatorGroupInteraction::Session(message) => self.update_session(message),
            IndicatorGroupInteraction::AppearancePresets(AppearancePresetsMessage::Apply(
                preset,
            )) => self.indicator_groups.apply_appearance_preset(preset),
            IndicatorGroupInteraction::AppearancePresets(AppearancePresetsMessage::Expansion(
                expansion,
            )) => {
                self.indicator_groups
                    .appearance_presets_group
                    .update_expansion(expansion);
                Task::none()
            }
            IndicatorGroupInteraction::Media(interaction) => self.interact_media(interaction),
            IndicatorGroupInteraction::Tray(message) => self.update_tray(message),
            IndicatorGroupInteraction::WindowMenuBar(interaction) => {
                self.interact_window_menu_bar(interaction)
            }
            IndicatorGroupInteraction::VirtualMachines(interaction) => {
                self.interact_virtual_machines(interaction)
            }
            IndicatorGroupInteraction::GroupMenus(message) => self.update_group_menus(message),
            indicator_group_interaction => self.indicator_groups.interact(
                indicator_group_interaction,
                &self.config,
                self.clock,
            ),
        }
    }

    pub(crate) fn update_group_menus(&mut self, message: GroupMenuMessage) -> Task<Message> {
        self.indicator_groups
            .group_menus
            .update(message)
            .map(Message::GroupMenuState)
    }
}
