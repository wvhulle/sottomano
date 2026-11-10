mod sqlite_panel_layout;

use iced::Task;
use sottomano_toml_config::Config;

use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
        window_group::WindowRowLayout,
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, ShellLayer},
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

impl From<PanelGroupFraming> for SwitchPosition {
    fn from(group_framing: PanelGroupFraming) -> Self {
        match group_framing {
            PanelGroupFraming::Disconnected => SwitchPosition::On,
            PanelGroupFraming::Joined => SwitchPosition::Off,
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

impl From<GroupCollapsing> for SwitchPosition {
    fn from(group_collapsing: GroupCollapsing) -> Self {
        match group_collapsing {
            GroupCollapsing::StayExpanded => SwitchPosition::On,
            GroupCollapsing::OnFocusLoss => SwitchPosition::Off,
        }
    }
}

impl SingletonFlagState for WindowRowLayout {
    const STORED_FLAG: SingletonFlag = SingletonFlag::WindowsGroupedByLocation;
    const STORED_AS_SET: Self = WindowRowLayout::GroupedByLocation;
    const STORED_AS_CLEARED: Self = WindowRowLayout::Flat;
}

impl From<WindowRowLayout> for SwitchPosition {
    fn from(window_row_layout: WindowRowLayout) -> Self {
        match window_row_layout {
            WindowRowLayout::GroupedByLocation => SwitchPosition::On,
            WindowRowLayout::Flat => SwitchPosition::Off,
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

fn disconnected_toggle(group_framing: PanelGroupFraming) -> IconToggle {
    IconToggle::new(
        "Disconnected groups",
        Phosphor::Pill(Weight::Regular),
        group_framing,
        SwitchPresses {
            turning_on: "draw each group in its own frame",
            turning_off: "join the groups into one panel",
        },
    )
}

fn pinned_groups_toggle(group_collapsing: GroupCollapsing) -> IconToggle {
    IconToggle::new(
        "Stay expanded",
        Phosphor::PushPin(Weight::Regular),
        group_collapsing,
        SwitchPresses {
            turning_on: "keep groups open until their handle is clicked",
            turning_off: "collapse groups when another window takes focus",
        },
    )
}

fn window_row_layout_toggle(window_row_layout: WindowRowLayout) -> IconToggle {
    IconToggle::new(
        "Group windows by directory",
        Phosphor::Folders(Weight::Regular),
        window_row_layout,
        SwitchPresses {
            turning_on: "enclose neighbouring windows from one directory under one heading",
            turning_off: "show every window on its own with its directory",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSettingsTarget {
    DisconnectedGroups,
    AutoCollapseGroups,
    WindowRowLayout,
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
    ToggleWindowRowLayout,
    FloatOffset(ControlMessage),
    Appearance(AppearanceMessage),
    LayoutLoaded(PanelLayout),
    WindowRowLayoutLoaded(WindowRowLayout),
}

pub struct PanelSettingsGroup {
    expansion: WidgetGroupExpansion,
    group_framing: PanelGroupFraming,
    group_collapsing: GroupCollapsing,
    window_row_layout: WindowRowLayout,
    float_offset: SliderControl,
    appearance: LayerAppearanceControls,
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        let defaults = PanelLayout::default();
        let mut panel_settings_group = Self {
            expansion: WidgetGroupExpansion::default(),
            group_framing: defaults.group_framing,
            group_collapsing: defaults.group_collapsing,
            window_row_layout: WindowRowLayout::default(),
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
            Task::perform(
                sqlite_singleton_flag::load(WindowRowLayout::GroupedByLocation),
                PanelSettingsMessage::WindowRowLayoutLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Panels).map(PanelSettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn group_framing(&self) -> PanelGroupFraming {
        self.group_framing
    }

    #[must_use]
    pub fn group_collapsing(&self) -> GroupCollapsing {
        self.group_collapsing
    }

    #[must_use]
    pub fn window_row_layout(&self) -> WindowRowLayout {
        self.window_row_layout
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
                disconnected_toggle(self.group_framing).switch_hover_card(clock)
            }
            PanelSettingsTarget::AutoCollapseGroups => {
                pinned_groups_toggle(self.group_collapsing).switch_hover_card(clock)
            }
            PanelSettingsTarget::WindowRowLayout => {
                window_row_layout_toggle(self.window_row_layout).switch_hover_card(clock)
            }
            PanelSettingsTarget::FloatOffset => self.float_offset.readout_hover_card("Offset"),
            PanelSettingsTarget::Appearance(target) => {
                self.appearance.hover_card_for(target, clock)
            }
        }
    }

    fn stored_layout(&self) -> PanelLayout {
        PanelLayout {
            group_framing: self.group_framing,
            group_collapsing: self.group_collapsing,
            float_offset_px: self.float_offset.settled_value(),
        }
    }

    fn adopt_layout(&mut self, panel_layout: PanelLayout) {
        self.group_framing = panel_layout.group_framing;
        self.group_collapsing = panel_layout.group_collapsing;
        self.float_offset
            .update(ControlMessage::Reading(panel_layout.float_offset_px));
    }

    fn close_sliders(&mut self) {
        self.float_offset.close();
        self.appearance.close();
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
            .map(move |store_outcome| {
                PanelSettingsMessage::FloatOffset(ControlMessage::persisted(value, store_outcome))
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
                    disconnected_toggle(self.group_framing),
                    PanelSettingsTarget::DisconnectedGroups,
                    PanelSettingsMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    pinned_groups_toggle(self.group_collapsing),
                    PanelSettingsTarget::AutoCollapseGroups,
                    PanelSettingsMessage::ToggleAutoCollapseGroups,
                ),
                GroupItem::toggle(
                    window_row_layout_toggle(self.window_row_layout),
                    PanelSettingsTarget::WindowRowLayout,
                    PanelSettingsMessage::ToggleWindowRowLayout,
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
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{
        GroupCollapsing, PanelGroupFraming, PanelLayout, PanelSettingsGroup, PanelSettingsMessage,
        sqlite_panel_layout::{read_layout, write_layout},
    };
    use crate::widget_groups::window_group::WindowRowLayout;

    #[tokio::test]
    async fn panel_layout_survives_a_round_trip() {
        let connection = migrated_in_memory_store().await;
        let stored = PanelLayout {
            group_framing: PanelGroupFraming::Disconnected,
            group_collapsing: GroupCollapsing::StayExpanded,
            float_offset_px: 3.25,
        };
        write_layout(&connection, stored).await.unwrap();
        assert_eq!(read_layout(&connection).await.unwrap(), stored);
    }

    #[tokio::test]
    async fn an_unwritten_panel_layout_keeps_its_defaults() {
        let connection = migrated_in_memory_store().await;
        assert_eq!(
            read_layout(&connection).await.unwrap(),
            PanelLayout::default()
        );
    }

    #[tokio::test]
    async fn the_panel_layout_table_refuses_a_missing_value() {
        let connection = migrated_in_memory_store().await;
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
    fn windows_are_grouped_by_location_until_a_stored_layout_says_otherwise() {
        let mut panel_settings_group = PanelSettingsGroup::default();
        assert_eq!(
            panel_settings_group.window_row_layout(),
            WindowRowLayout::GroupedByLocation
        );
        let _ = panel_settings_group.update(PanelSettingsMessage::WindowRowLayoutLoaded(
            WindowRowLayout::Flat,
        ));
        assert_eq!(
            panel_settings_group.window_row_layout(),
            WindowRowLayout::Flat
        );
    }

    #[test]
    fn a_loaded_panel_layout_reads_back_unchanged() {
        let loaded = PanelLayout {
            group_framing: PanelGroupFraming::Disconnected,
            group_collapsing: GroupCollapsing::StayExpanded,
            float_offset_px: 6.0,
        };
        let mut panel_settings_group = PanelSettingsGroup::default();
        let _ = panel_settings_group.update(PanelSettingsMessage::LayoutLoaded(loaded));
        assert_eq!(
            panel_settings_group.group_framing(),
            PanelGroupFraming::Disconnected
        );
        assert_eq!(
            panel_settings_group.group_collapsing(),
            GroupCollapsing::StayExpanded
        );
        assert!((panel_settings_group.float_offset_px() - 6.0).abs() < f32::EPSILON);
    }
}
