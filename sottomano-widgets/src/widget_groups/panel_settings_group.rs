use iced::Task;
use sottomano_config::Config;
use sottomano_sources::sqlite_state_store::Columns;

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::sqlite_shell_settings_row::{self, ShellSetting},
    widgets::{
        group_item::GroupItem,
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, ShellLayer},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLayout {
    pub disconnected_groups: bool,
    pub auto_collapse_groups: bool,
    pub float_offset_px: f32,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            disconnected_groups: false,
            auto_collapse_groups: true,
            float_offset_px: 0.0,
        }
    }
}

async fn load_panel_layout() -> PanelLayout {
    sqlite_shell_settings_row::load_stored_row(
        "shell settings: load panel layout failed",
        PanelLayout::default(),
        panel_layout_from_columns,
    )
    .await
}

fn panel_layout_from_columns(
    columns: &Columns,
    row: &turso::Row,
    defaults: PanelLayout,
) -> PanelLayout {
    PanelLayout {
        float_offset_px: columns
            .optional_real(row, "float_offset_px")
            .map_or(defaults.float_offset_px, |offset| offset as f32),
        disconnected_groups: columns
            .optional_flag(row, "disconnected_groups")
            .unwrap_or(defaults.disconnected_groups),
        auto_collapse_groups: columns
            .optional_flag(row, "auto_collapse_groups")
            .unwrap_or(defaults.auto_collapse_groups),
    }
}

pub enum Action {
    Run(Task<PanelSettingsMessage>),
    Reflow(Task<PanelSettingsMessage>),
}

fn disconnected_toggle(groups_disconnected: bool) -> IconToggle {
    IconToggle::new(
        "Disconnected groups",
        Phosphor::Pill(Weight::Regular),
        groups_disconnected,
        SwitchPresses {
            turning_on: "draw each group in its own frame",
            turning_off: "join the groups into one bar",
        },
    )
}

fn pinned_groups_toggle(groups_auto_collapse: bool) -> IconToggle {
    IconToggle::new(
        "Pinned groups",
        Phosphor::PushPin(Weight::Regular),
        !groups_auto_collapse,
        SwitchPresses {
            turning_on: "keep groups open until their handle is clicked",
            turning_off: "collapse groups when another window takes focus",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSettingsTarget {
    DisconnectedGroups,
    AutoCollapseGroups,
    FloatOffset,
    Appearance(AppearanceTarget),
}

impl From<PanelSettingsTarget> for HoverTarget {
    fn from(target: PanelSettingsTarget) -> Self {
        HoverTarget::PanelSettings(target)
    }
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Expansion(ExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    FloatOffset(ControlMessage),
    Appearance(AppearanceMessage),
    LayoutLoaded(PanelLayout),
}

pub struct PanelSettingsGroup {
    expansion: GroupExpansion,
    disconnected_groups: bool,
    auto_collapse_groups: bool,
    float_offset: SliderControl,
    appearance: LayerAppearanceControls,
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        let defaults = PanelLayout::default();
        let mut panel_settings_group = Self {
            expansion: GroupExpansion::default(),
            disconnected_groups: defaults.disconnected_groups,
            auto_collapse_groups: defaults.auto_collapse_groups,
            float_offset: SliderControl::new(
                "Float offset",
                0.0..=24.0,
                PanelSettingsTarget::FloatOffset,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            appearance: LayerAppearanceControls::new(ShellLayer::Panels, |target| {
                PanelSettingsTarget::Appearance(target).into()
            }),
        };
        panel_settings_group.adopt_layout(defaults);
        panel_settings_group
    }
}

impl PanelSettingsGroup {
    pub fn load() -> Task<PanelSettingsMessage> {
        Task::batch([
            Task::perform(load_panel_layout(), PanelSettingsMessage::LayoutLoaded),
            LayerAppearanceControls::load(ShellLayer::Panels).map(PanelSettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn groups_disconnected(&self) -> bool {
        self.disconnected_groups
    }

    #[must_use]
    pub fn groups_auto_collapse(&self) -> bool {
        self.auto_collapse_groups
    }

    #[must_use]
    pub fn float_offset_px(&self) -> f32 {
        self.float_offset.settled_value()
    }

    #[must_use]
    pub fn panel_appearance(&self) -> LayerAppearance {
        self.appearance.appearance()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: PanelSettingsTarget, clock: Clock) -> Tooltip {
        match target {
            PanelSettingsTarget::DisconnectedGroups => {
                disconnected_toggle(self.disconnected_groups).switch_tooltip(clock)
            }
            PanelSettingsTarget::AutoCollapseGroups => {
                pinned_groups_toggle(self.auto_collapse_groups).switch_tooltip(clock)
            }
            PanelSettingsTarget::FloatOffset => self.float_offset.readout_tooltip("Offset"),
            PanelSettingsTarget::Appearance(target) => self.appearance.tooltip_for(target, clock),
        }
    }

    fn adopt_layout(&mut self, panel_layout: PanelLayout) {
        self.disconnected_groups = panel_layout.disconnected_groups;
        self.auto_collapse_groups = panel_layout.auto_collapse_groups;
        self.float_offset
            .update(ControlMessage::Reading(panel_layout.float_offset_px));
    }

    fn close_sliders(&mut self) {
        self.float_offset.close();
        self.appearance.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: PanelSettingsMessage) -> Action {
        match msg {
            PanelSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Action::Run(Task::none())
            }
            PanelSettingsMessage::ToggleDisconnectedGroups => {
                self.disconnected_groups = !self.disconnected_groups;
                Action::Run(
                    Task::future(sqlite_shell_settings_row::save(
                        ShellSetting::DisconnectedGroups(self.disconnected_groups),
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                self.auto_collapse_groups = !self.auto_collapse_groups;
                Action::Run(
                    Task::future(sqlite_shell_settings_row::save(
                        ShellSetting::AutoCollapseGroups(self.auto_collapse_groups),
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::FloatOffset(control) => self.update_float_offset(control),
            PanelSettingsMessage::Appearance(msg) => Action::Run(
                self.appearance
                    .update(msg)
                    .map(PanelSettingsMessage::Appearance),
            ),
            PanelSettingsMessage::LayoutLoaded(panel_layout) => {
                self.adopt_layout(panel_layout);
                Action::Reflow(Task::none())
            }
        }
    }

    #[allow(clippy::float_cmp)]
    fn update_float_offset(&mut self, control: ControlMessage) -> Action {
        let settled_before = self.float_offset.settled_value();
        let persisting = match self.float_offset.update(control) {
            SliderAction::Commit(value) => Task::future(sqlite_shell_settings_row::save(
                ShellSetting::FloatOffset(value),
            ))
            .map(move |ok| {
                PanelSettingsMessage::FloatOffset(ControlMessage::Persisted { value, ok })
            }),
            SliderAction::None
            | SliderAction::Opened
            | SliderAction::Adjust(_)
            | SliderAction::Right => Task::none(),
        };
        if self.float_offset.settled_value() == settled_before {
            Action::Run(persisting)
        } else {
            Action::Reflow(persisting)
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, PanelSettingsMessage> {
        GroupContent::new(
            &self.expansion,
            PanelSettingsMessage::Expansion,
            [
                GroupItem::toggle(
                    disconnected_toggle(self.disconnected_groups),
                    PanelSettingsTarget::DisconnectedGroups,
                    PanelSettingsMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    pinned_groups_toggle(self.auto_collapse_groups),
                    PanelSettingsTarget::AutoCollapseGroups,
                    PanelSettingsMessage::ToggleAutoCollapseGroups,
                ),
                GroupItem::slider(
                    &self.float_offset,
                    config,
                    PanelSettingsMessage::FloatOffset,
                ),
            ]
            .into_iter()
            .chain(
                self.appearance
                    .items(config, PanelSettingsMessage::Appearance),
            ),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Rows(Weight::Regular).into(),
            handle: GroupHandle::PanelSettings,
        })
    }
}

#[cfg(test)]
mod tests {
    use iced::futures::{StreamExt, stream};
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{PanelLayout, PanelSettingsGroup, PanelSettingsMessage, panel_layout_from_columns};
    use crate::widget_groups::sqlite_shell_settings_row::{
        CREATE_TABLE, ShellSetting, read_stored_row, write_setting,
    };

    async fn stored_panel_layout(connection: &turso::Connection) -> PanelLayout {
        read_stored_row(
            connection,
            PanelLayout::default(),
            panel_layout_from_columns,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn panel_layout_settings_round_trip_to_their_own_columns() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        stream::iter([
            ShellSetting::FloatOffset(3.25),
            ShellSetting::DisconnectedGroups(true),
            ShellSetting::AutoCollapseGroups(false),
        ])
        .for_each(async |setting| write_setting(&connection, setting).await.unwrap())
        .await;

        let panel_layout = stored_panel_layout(&connection).await;
        assert!((panel_layout.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!(panel_layout.disconnected_groups);
        assert!(!panel_layout.auto_collapse_groups);
    }

    #[tokio::test]
    async fn an_unwritten_panel_layout_keeps_its_defaults() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(&connection, ShellSetting::Sound(false))
            .await
            .unwrap();

        assert_eq!(
            stored_panel_layout(&connection).await,
            PanelLayout::default()
        );
    }

    #[test]
    fn a_loaded_panel_layout_reads_back_unchanged() {
        let loaded = PanelLayout {
            disconnected_groups: true,
            auto_collapse_groups: false,
            float_offset_px: 6.0,
        };
        let mut panel_settings_group = PanelSettingsGroup::default();
        let _ = panel_settings_group.update(PanelSettingsMessage::LayoutLoaded(loaded));
        assert!(panel_settings_group.groups_disconnected());
        assert!(!panel_settings_group.groups_auto_collapse());
        assert!((panel_settings_group.float_offset_px() - 6.0).abs() < f32::EPSILON);
    }
}
