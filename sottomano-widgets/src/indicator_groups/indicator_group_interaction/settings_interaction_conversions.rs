use super::IndicatorGroupInteraction;
use crate::indicator_groups::{
    appearance_presets_group::AppearancePresetsMessage,
    compositor_styling_group::CompositorStylingMessage,
    overlay_settings_group::OverlaySettingsMessage, panel_settings_group::PanelSettingsMessage,
    sidebar_settings_group::SidebarSettingsMessage,
};

impl From<PanelSettingsMessage> for IndicatorGroupInteraction {
    fn from(interaction: PanelSettingsMessage) -> Self {
        IndicatorGroupInteraction::PanelSettings(interaction)
    }
}

impl From<SidebarSettingsMessage> for IndicatorGroupInteraction {
    fn from(interaction: SidebarSettingsMessage) -> Self {
        IndicatorGroupInteraction::SidebarSettings(interaction)
    }
}

impl From<AppearancePresetsMessage> for IndicatorGroupInteraction {
    fn from(interaction: AppearancePresetsMessage) -> Self {
        IndicatorGroupInteraction::AppearancePresets(interaction)
    }
}

impl From<OverlaySettingsMessage> for IndicatorGroupInteraction {
    fn from(interaction: OverlaySettingsMessage) -> Self {
        IndicatorGroupInteraction::OverlaySettings(interaction)
    }
}

impl From<CompositorStylingMessage> for IndicatorGroupInteraction {
    fn from(interaction: CompositorStylingMessage) -> Self {
        IndicatorGroupInteraction::CompositorStyling(interaction)
    }
}
