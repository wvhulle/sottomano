use iced::Task;
use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::CompositorRequest;
use sottomano_widgets::{
    background_work::{compositor_request, copy_to_clipboard},
    group_menus::group_menu_state::{GroupMenuMessage, PanelPin, PanelPinDuration},
    hover_cards::PopupMessage,
    widget_groups::{
        appearance_presets_group::AppearancePresetsMessage,
        sidebar_settings_group::SidebarSettingsMessage,
        widget_group_interaction::{WidgetGroupBusRequest, WidgetGroupInteraction},
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
                self.widget_groups.collapse_when_auto_collapsing();
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
                self.interact_shell_group(WidgetGroupInteraction::SidebarSettings(
                    SidebarSettingsMessage::ToggleEmptyWorkspaces,
                ))
            }
            WorkspacesMessage::Focus(workspace_id) => {
                compositor_request(CompositorRequest::FocusWorkspace(workspace_id))
            }
        }
    }

    fn interact_group(&mut self, group_interaction: WidgetGroupInteraction) -> Task<Message> {
        match WidgetGroupBusRequest::try_from(group_interaction) {
            Ok(bus_request) => {
                let Some(buses) = &self.desktop_buses else {
                    return Task::none();
                };
                self.widget_groups
                    .request_on_buses(bus_request, &self.config, buses, self.clock)
            }
            Err(local_interaction) => self.interact_shell_group(local_interaction),
        }
    }

    fn interact_shell_group(&mut self, group_interaction: WidgetGroupInteraction) -> Task<Message> {
        match group_interaction {
            WidgetGroupInteraction::WindowGroup(interaction) => {
                self.interact_window_group(interaction)
            }
            WidgetGroupInteraction::AppLauncher(message) => self.update_app_launcher(message),
            WidgetGroupInteraction::Gamepad(interaction) => self.interact_gamepad(interaction),
            WidgetGroupInteraction::PanelSettings(message) => self.update_panel_settings(message),
            WidgetGroupInteraction::OverlaySettings(message) => {
                self.update_overlay_settings(message)
            }
            WidgetGroupInteraction::NotificationGroup(message) => {
                self.update_notification_group(message)
            }
            WidgetGroupInteraction::QuietSchedule(message) => self.update_quiet_schedule(message),
            WidgetGroupInteraction::Session(message) => self.update_session(message),
            WidgetGroupInteraction::AppearancePresets(AppearancePresetsMessage::Apply(preset)) => {
                self.widget_groups.apply_appearance_preset(preset)
            }
            WidgetGroupInteraction::AppearancePresets(AppearancePresetsMessage::Expansion(
                expansion,
            )) => {
                self.widget_groups
                    .appearance_presets_group
                    .update_expansion(expansion);
                Task::none()
            }
            WidgetGroupInteraction::Media(interaction) => self.interact_media(interaction),
            WidgetGroupInteraction::Tray(message) => self.update_tray(message),
            WidgetGroupInteraction::GlobalMenu(interaction) => {
                self.interact_global_menu(interaction)
            }
            WidgetGroupInteraction::VirtualMachines(interaction) => {
                self.interact_virtual_machines(interaction)
            }
            WidgetGroupInteraction::GroupMenus(message) => self.update_group_menus(message),
            widget_group_interaction => {
                self.widget_groups
                    .interact(widget_group_interaction, &self.config, self.clock)
            }
        }
    }

    pub(crate) fn update_group_menus(&mut self, message: GroupMenuMessage) -> Task<Message> {
        self.widget_groups
            .group_menus
            .update(message)
            .map(Message::GroupMenuState)
    }
}
