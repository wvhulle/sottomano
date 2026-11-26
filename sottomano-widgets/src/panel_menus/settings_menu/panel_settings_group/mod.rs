#[cfg(test)]
mod panel_settings_tests;
mod setting_switch_toggles;
mod sqlite_panel_layout;

use iced::Task;
use sottomano_toml_config::Config;

use self::setting_switch_toggles::{
    disconnected_toggle, pinned_groups_toggle, window_row_layout_toggle, window_title_length_toggle,
};
use crate::{
    background_work::logged_failure_task,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            ExpansionChange, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage, IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        sqlite_singleton_flag,
    },
    output_windows::{WindowRowLayout, WindowTitleLength},
    widgets::{
        layer_appearance_controls::{AppearanceMessage, LayerAppearanceControls},
        shell_layer_appearance::ShellLayer,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelGroupFraming {
    Joined,
    Disconnected,
}

impl PanelGroupFraming {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            PanelGroupFraming::Joined => PanelGroupFraming::Disconnected,
            PanelGroupFraming::Disconnected => PanelGroupFraming::Joined,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupCollapsing {
    OnFocusLoss,
    StayExpanded,
}

impl GroupCollapsing {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            GroupCollapsing::OnFocusLoss => GroupCollapsing::StayExpanded,
            GroupCollapsing::StayExpanded => GroupCollapsing::OnFocusLoss,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLayout {
    pub group_framing: PanelGroupFraming,
    pub group_collapsing: GroupCollapsing,
    pub float_offset_px: f32,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            group_framing: PanelGroupFraming::Joined,
            group_collapsing: GroupCollapsing::OnFocusLoss,
            float_offset_px: 0.0,
        }
    }
}

pub enum Action {
    Run(Task<PanelSettingsMessage>),
    Reflow(Task<PanelSettingsMessage>),
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Expansion(IndicatorGroupExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    ToggleWindowRowLayout,
    ToggleWindowTitleLength,
    FloatOffset(ControlMessage),
    Appearance(AppearanceMessage),
    LayoutLoaded(PanelLayout),
    WindowRowLayoutLoaded(WindowRowLayout),
    WindowTitleLengthLoaded(WindowTitleLength),
}

pub struct PanelSettingsGroup {
    expansion: IndicatorGroupExpansion,
    pub group_framing: PanelGroupFraming,
    pub group_collapsing: GroupCollapsing,
    pub window_row_layout: WindowRowLayout,
    pub window_title_length: WindowTitleLength,
    float_offset: SliderControl,
    pub layer: LayerAppearanceControls,
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        let defaults = PanelLayout::default();
        let mut panel_settings_group = Self {
            expansion: IndicatorGroupExpansion::default(),
            group_framing: defaults.group_framing,
            group_collapsing: defaults.group_collapsing,
            window_row_layout: WindowRowLayout::default(),
            window_title_length: WindowTitleLength::default(),
            float_offset: SliderControl::new(
                "Float offset",
                0.0..=24.0,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            layer: LayerAppearanceControls::new(ShellLayer::Panels),
        };
        panel_settings_group.adopt_layout(defaults);
        panel_settings_group
    }
}

impl PanelSettingsGroup {
    pub fn load() -> Task<PanelSettingsMessage> {
        Task::batch([
            Task::perform(
                sqlite_panel_layout::load(),
                PanelSettingsMessage::LayoutLoaded,
            ),
            Task::perform(
                sqlite_singleton_flag::load(WindowRowLayout::GroupedByLocation),
                PanelSettingsMessage::WindowRowLayoutLoaded,
            ),
            Task::perform(
                sqlite_singleton_flag::load(WindowTitleLength::Capped),
                PanelSettingsMessage::WindowTitleLengthLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Panels).map(PanelSettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn float_offset_px(&self) -> f32 {
        self.float_offset
            .expanding_slider
            .synced_value
            .reported_value()
    }

    fn stored_layout(&self) -> PanelLayout {
        PanelLayout {
            group_framing: self.group_framing,
            group_collapsing: self.group_collapsing,
            float_offset_px: self
                .float_offset
                .expanding_slider
                .synced_value
                .reported_value(),
        }
    }

    fn adopt_layout(&mut self, panel_layout: PanelLayout) {
        self.group_framing = panel_layout.group_framing;
        self.group_collapsing = panel_layout.group_collapsing;
        self.float_offset
            .update(ControlMessage::Sample(panel_layout.float_offset_px));
    }

    fn close_sliders(&mut self) {
        self.float_offset.close();
        self.layer.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: PanelSettingsMessage) -> Action {
        match msg {
            PanelSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) == ExpansionChange::JustCollapsed {
                    self.close_sliders();
                }
                Action::Run(Task::none())
            }
            PanelSettingsMessage::ToggleDisconnectedGroups => {
                self.group_framing = self.group_framing.toggled();
                Action::Run(logged_failure_task(
                    "panel layout: save".to_owned(),
                    sqlite_panel_layout::save(self.stored_layout()),
                ))
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                self.group_collapsing = self.group_collapsing.toggled();
                Action::Run(logged_failure_task(
                    "panel layout: save".to_owned(),
                    sqlite_panel_layout::save(self.stored_layout()),
                ))
            }
            PanelSettingsMessage::ToggleWindowRowLayout => {
                self.window_row_layout = match self.window_row_layout {
                    WindowRowLayout::Flat => WindowRowLayout::GroupedByLocation,
                    WindowRowLayout::GroupedByLocation => WindowRowLayout::Flat,
                };
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.window_row_layout),
                ))
            }
            PanelSettingsMessage::WindowRowLayoutLoaded(window_row_layout) => {
                self.window_row_layout = window_row_layout;
                Action::Run(Task::none())
            }
            PanelSettingsMessage::ToggleWindowTitleLength => {
                self.window_title_length = match self.window_title_length {
                    WindowTitleLength::Capped => WindowTitleLength::Full,
                    WindowTitleLength::Full => WindowTitleLength::Capped,
                };
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.window_title_length),
                ))
            }
            PanelSettingsMessage::WindowTitleLengthLoaded(window_title_length) => {
                self.window_title_length = window_title_length;
                Action::Run(Task::none())
            }
            PanelSettingsMessage::FloatOffset(control) => self.update_float_offset(control),
            PanelSettingsMessage::Appearance(msg) => {
                Action::Run(self.layer.update(msg).map(PanelSettingsMessage::Appearance))
            }
            PanelSettingsMessage::LayoutLoaded(panel_layout) => {
                self.adopt_layout(panel_layout);
                Action::Reflow(Task::none())
            }
        }
    }

    #[allow(clippy::float_cmp)]
    fn update_float_offset(&mut self, control: ControlMessage) -> Action {
        let settled_before = self
            .float_offset
            .expanding_slider
            .synced_value
            .reported_value();
        let persisting = match self.float_offset.update(control) {
            SliderAction::Commit(value) => Task::future(sqlite_panel_layout::save(PanelLayout {
                float_offset_px: value,
                ..self.stored_layout()
            }))
            .map(move |store_outcome| {
                PanelSettingsMessage::FloatOffset(ControlMessage::persisted(value, store_outcome))
            }),
            SliderAction::None
            | SliderAction::Opened
            | SliderAction::Adjust(_)
            | SliderAction::Right => Task::none(),
        };
        if self
            .float_offset
            .expanding_slider
            .synced_value
            .reported_value()
            == settled_before
        {
            Action::Run(persisting)
        } else {
            Action::Reflow(persisting)
        }
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::PanelSettings,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::PanelSettings.title(),
                Phosphor::Rows(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            PanelSettingsMessage::Expansion,
            IndicatorListing::Listed(
                self.layer
                    .indicators(config, PanelSettingsMessage::Appearance)
                    .into_iter()
                    .chain([
                        GroupIndicator::slider(
                            &self.float_offset,
                            config,
                            PanelSettingsMessage::FloatOffset,
                        ),
                        GroupIndicator::toggle(
                            disconnected_toggle(self.group_framing),
                            PanelSettingsMessage::ToggleDisconnectedGroups,
                        ),
                        GroupIndicator::toggle(
                            pinned_groups_toggle(self.group_collapsing),
                            PanelSettingsMessage::ToggleAutoCollapseGroups,
                        ),
                        GroupIndicator::toggle(
                            window_row_layout_toggle(self.window_row_layout),
                            PanelSettingsMessage::ToggleWindowRowLayout,
                        ),
                        GroupIndicator::toggle(
                            window_title_length_toggle(self.window_title_length),
                            PanelSettingsMessage::ToggleWindowTitleLength,
                        ),
                    ])
                    .collect(),
            ),
        )
    }
}
