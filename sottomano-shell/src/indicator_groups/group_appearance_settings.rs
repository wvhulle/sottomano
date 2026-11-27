use corredo::popovers::PopupEdgeClearance;
use iced::Task;
use sottomano_compositor_model::compositor_window_state::CompositorWindowState;
use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock,
    group_indicators::indicator_group_interaction::IndicatorGroupInteraction,
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{anchored_popup::PopupAppearances, hover_card_surfaces::HoverCardSurfaceStyle},
    notification_toasts::{NotificationSettings, ToastContext},
    panel_menus::{
        display_menu::dark_mode_group::DarkModeInteraction,
        settings_menu::{
            overlay_settings_group::OverlaySettingsMessage,
            panel_settings_group::PanelSettingsMessage,
            sidebar_settings_group::SidebarSettingsMessage,
        },
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        layer_appearance_controls::AppearanceMessage,
    },
};

use crate::{
    indicator_groups::ShellIndicatorGroups,
    shell_message::{Message, ShellInteraction},
};

impl ShellIndicatorGroups {
    #[must_use]
    pub fn shell_appearance(&self) -> ShellAppearance {
        ShellAppearance {
            scheme: self.panel_menus.display.dark_mode_group.scheme(),
            panels: self
                .panel_menus
                .settings
                .panel_settings_group
                .layer
                .appearance(),
            sidebar: self
                .panel_menus
                .settings
                .sidebar_settings_group
                .layer
                .appearance(),
            overlays: self
                .panel_menus
                .settings
                .overlay_settings_group
                .layer
                .appearance(),
            menus: self.group_menus.layer.appearance(),
        }
    }

    pub fn apply_appearance_preset(&self, preset: AppearancePreset) -> Task<Message> {
        let current = self.shell_appearance();
        let next = preset.applied_to(current);
        let scheme_switch = (next.scheme != current.scheme).then(|| {
            Task::done(Message::from(ShellInteraction::from(
                IndicatorGroupInteraction::DarkMode(DarkModeInteraction::SwitchScheme(next.scheme)),
            )))
        });
        Task::batch(
            [
                Task::done(Message::PanelSettings(PanelSettingsMessage::Appearance(
                    AppearanceMessage::Adopted(next.panels),
                ))),
                Task::done(Message::SidebarSettings(
                    SidebarSettingsMessage::Appearance(AppearanceMessage::Adopted(next.sidebar)),
                )),
                Task::done(Message::OverlaySettings(
                    OverlaySettingsMessage::Appearance(AppearanceMessage::Adopted(next.overlays)),
                )),
                Task::done(Message::GroupMenuState(GroupMenuMessage::Appearance(
                    AppearanceMessage::Adopted(next.menus),
                ))),
            ]
            .into_iter()
            .chain(scheme_switch),
        )
    }

    #[must_use]
    pub fn notification_settings(&self) -> NotificationSettings {
        NotificationSettings {
            alert_sound: self.panel_menus.agenda.notification_group.alert_sound,
            toast_arrangement: self
                .panel_menus
                .settings
                .overlay_settings_group
                .toast_arrangement,
        }
    }

    #[must_use]
    pub fn hover_card_surface_style(&self) -> HoverCardSurfaceStyle {
        HoverCardSurfaceStyle {
            popup: PopupAppearances {
                edge_clearance: PopupEdgeClearance::FlushAgainstEdge,
                overlay: self
                    .panel_menus
                    .settings
                    .overlay_settings_group
                    .layer
                    .appearance(),
                menu: self.group_menus.layer.appearance(),
            },
            float_offset_px: self
                .panel_menus
                .settings
                .panel_settings_group
                .float_offset_px(),
        }
    }

    #[must_use]
    pub fn toast_context<'shell>(
        &self,
        compositor: &CompositorWindowState,
        config: &'shell Config,
        clock: Clock,
    ) -> ToastContext<'shell> {
        ToastContext::new(
            compositor,
            config,
            self.notification_settings(),
            self.panel_menus.agenda.quiet_schedule_group.phase,
            clock,
        )
    }
}
