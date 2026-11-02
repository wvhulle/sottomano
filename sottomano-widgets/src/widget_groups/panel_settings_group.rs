mod sqlite_panel_layout;

use iced::Task;
use sottomano_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, ShellLayer},
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
            turning_off: "join the groups into one panel",
        },
    )
}

fn pinned_groups_toggle(groups_auto_collapse: bool) -> IconToggle {
    IconToggle::new(
        "Stay expanded",
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
    Expansion(WidgetGroupExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    FloatOffset(ControlMessage),
    Appearance(AppearanceMessage),
    LayoutLoaded(PanelLayout),
}

pub struct PanelSettingsGroup {
    expansion: WidgetGroupExpansion,
    disconnected_groups: bool,
    auto_collapse_groups: bool,
    float_offset: SliderControl,
    appearance: LayerAppearanceControls,
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        let defaults = PanelLayout::default();
        let mut panel_settings_group = Self {
            expansion: WidgetGroupExpansion::default(),
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
            Task::perform(
                sqlite_panel_layout::load(),
                PanelSettingsMessage::LayoutLoaded,
            ),
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
    pub fn hover_card_for(&self, target: PanelSettingsTarget, clock: Clock) -> HoverCard {
        match target {
            PanelSettingsTarget::DisconnectedGroups => {
                disconnected_toggle(self.disconnected_groups).switch_hover_card(clock)
            }
            PanelSettingsTarget::AutoCollapseGroups => {
                pinned_groups_toggle(self.auto_collapse_groups).switch_hover_card(clock)
            }
            PanelSettingsTarget::FloatOffset => self.float_offset.readout_hover_card("Offset"),
            PanelSettingsTarget::Appearance(target) => {
                self.appearance.hover_card_for(target, clock)
            }
        }
    }

    fn stored_layout(&self) -> PanelLayout {
        PanelLayout {
            disconnected_groups: self.disconnected_groups,
            auto_collapse_groups: self.auto_collapse_groups,
            float_offset_px: self.float_offset.settled_value(),
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
                Action::Run(Task::future(sqlite_panel_layout::save(self.stored_layout())).discard())
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                self.auto_collapse_groups = !self.auto_collapse_groups;
                Action::Run(Task::future(sqlite_panel_layout::save(self.stored_layout())).discard())
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
            SliderAction::Commit(value) => Task::future(sqlite_panel_layout::save(PanelLayout {
                float_offset_px: value,
                ..self.stored_layout()
            }))
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
    ) -> WidgetGroupContent<'a, PanelSettingsMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PanelSettings,
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
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Rows(Weight::Regular).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{
        PanelLayout, PanelSettingsGroup, PanelSettingsMessage,
        sqlite_panel_layout::{CREATE_TABLE, read_layout, write_layout},
    };

    #[tokio::test]
    async fn panel_layout_survives_a_round_trip() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let stored = PanelLayout {
            disconnected_groups: true,
            auto_collapse_groups: false,
            float_offset_px: 3.25,
        };
        write_layout(&connection, stored).await.unwrap();
        assert_eq!(read_layout(&connection).await.unwrap(), stored);
    }

    #[tokio::test]
    async fn an_unwritten_panel_layout_keeps_its_defaults() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        assert_eq!(
            read_layout(&connection).await.unwrap(),
            PanelLayout::default()
        );
    }

    #[tokio::test]
    async fn the_panel_layout_table_refuses_a_missing_value() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let refused = connection
            .execute(
                "INSERT INTO panel_layout (id, disconnected_groups, auto_collapse_groups, \
                 float_offset_px) VALUES (0, NULL, 1, 0.0)",
                (),
            )
            .await;
        assert!(refused.is_err());
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
