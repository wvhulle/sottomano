use std::time::Duration;

use iced::{Task, task};
use sottomano_desktop_sources::desktop_windows_apps::workspace_window_snapshot;
use sottomano_toml_config::Config;
use tokio::time::sleep;

use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
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
        workspaces_group::{EmptyWorkspaceFolding, EmptyWorkspaces, RevealFoldTimer},
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::ShellLayer,
    },
};

const REVEAL_FOLD_DELAY: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacesPlacement {
    Sidebar,
    BottomPanel,
}

impl WorkspacesPlacement {
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Sidebar => Self::BottomPanel,
            Self::BottomPanel => Self::Sidebar,
        }
    }
}

impl SingletonFlagState for WorkspacesPlacement {
    const STORED_FLAG: SingletonFlag = SingletonFlag::WorkspacesInSidebar;
    const STORED_AS_SET: Self = WorkspacesPlacement::Sidebar;
    const STORED_AS_CLEARED: Self = WorkspacesPlacement::BottomPanel;
}

impl From<WorkspacesPlacement> for SwitchPosition {
    fn from(placement: WorkspacesPlacement) -> Self {
        match placement {
            WorkspacesPlacement::Sidebar => SwitchPosition::On,
            WorkspacesPlacement::BottomPanel => SwitchPosition::Off,
        }
    }
}

fn workspaces_sidebar_toggle(placement: WorkspacesPlacement) -> IconToggle {
    IconToggle::new(
        "Workspaces sidebar",
        Phosphor::SidebarSimple(Weight::Regular),
        placement,
        SwitchPresses {
            turning_on: "move workspaces to a sidebar on the left edge",
            turning_off: "move workspaces inline into the bottom panel",
        },
    )
}

impl SingletonFlagState for EmptyWorkspaces {
    const STORED_FLAG: SingletonFlag = SingletonFlag::AutoCollapseWorkspaces;
    const STORED_AS_SET: Self = EmptyWorkspaces::Folded;
    const STORED_AS_CLEARED: Self = EmptyWorkspaces::Shown;
}

impl From<EmptyWorkspaces> for SwitchPosition {
    fn from(empty_workspaces_mode: EmptyWorkspaces) -> Self {
        match empty_workspaces_mode {
            EmptyWorkspaces::Folded => SwitchPosition::On,
            EmptyWorkspaces::Shown => SwitchPosition::Off,
        }
    }
}

fn auto_collapse_workspaces_toggle(empty_workspaces_mode: EmptyWorkspaces) -> IconToggle {
    IconToggle::new(
        "Auto-collapse unused workspaces",
        Phosphor::CaretUp(Weight::Regular),
        empty_workspaces_mode,
        SwitchPresses {
            turning_on: "fold empty workspaces away while they are unused",
            turning_off: "keep empty workspaces listed",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSettingsTarget {
    WorkspacesPlacement,
    AutoCollapseWorkspaces,
    Appearance(AppearanceTarget),
}

impl From<SidebarSettingsTarget> for HoverTarget {
    fn from(target: SidebarSettingsTarget) -> Self {
        HoverTarget::SidebarSettings(target)
    }
}

#[derive(Debug, Clone)]
pub enum SidebarSettingsMessage {
    Expansion(WidgetGroupExpansionMessage),
    ToggleWorkspacesPlacement,
    ToggleEmptyWorkspaces,
    ToggleAutoCollapseWorkspaces,
    FoldTemporaryReveal,
    Appearance(AppearanceMessage),
    PlacementLoaded(WorkspacesPlacement),
    EmptyWorkspacesModeLoaded(EmptyWorkspaces),
}

pub struct SidebarSettingsGroup {
    expansion: WidgetGroupExpansion,
    workspaces_placement: WorkspacesPlacement,
    empty_workspace_folding: EmptyWorkspaceFolding,
    reveal_fold_timer: Option<task::Handle>,
    pub layer: LayerAppearanceControls,
}

impl Default for SidebarSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: WidgetGroupExpansion::default(),
            workspaces_placement: WorkspacesPlacement::Sidebar,
            empty_workspace_folding: EmptyWorkspaceFolding::default(),
            reveal_fold_timer: None,
            layer: LayerAppearanceControls::new(ShellLayer::Sidebar, |target| {
                SidebarSettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl SidebarSettingsGroup {
    pub fn load() -> Task<SidebarSettingsMessage> {
        Task::batch([
            Task::perform(
                sqlite_singleton_flag::load(WorkspacesPlacement::Sidebar),
                SidebarSettingsMessage::PlacementLoaded,
            ),
            Task::perform(
                sqlite_singleton_flag::load(EmptyWorkspaces::Folded),
                SidebarSettingsMessage::EmptyWorkspacesModeLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Sidebar)
                .map(SidebarSettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn workspaces_placement(&self) -> WorkspacesPlacement {
        self.workspaces_placement
    }

    #[must_use]
    pub fn empty_workspace_folding(&self) -> &EmptyWorkspaceFolding {
        &self.empty_workspace_folding
    }

    pub fn observe_workspace_switch(
        &mut self,
        compositor: &workspace_window_snapshot::Snapshot,
    ) -> Task<SidebarSettingsMessage> {
        match self
            .empty_workspace_folding
            .observe_workspace_switch(compositor)
        {
            RevealFoldTimer::Keep => Task::none(),
            RevealFoldTimer::Stop => {
                self.reveal_fold_timer = None;
                Task::none()
            }
            RevealFoldTimer::Restart => {
                let (timer, handle) = Task::future(sleep(REVEAL_FOLD_DELAY))
                    .map(|()| SidebarSettingsMessage::FoldTemporaryReveal)
                    .abortable();
                self.reveal_fold_timer = Some(handle.abort_on_drop());
                timer
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: SidebarSettingsTarget, clock: Clock) -> HoverCard {
        match target {
            SidebarSettingsTarget::WorkspacesPlacement => {
                workspaces_sidebar_toggle(self.workspaces_placement).switch_hover_card(clock)
            }
            SidebarSettingsTarget::AutoCollapseWorkspaces => {
                auto_collapse_workspaces_toggle(self.empty_workspace_folding.mode())
                    .switch_hover_card(clock)
            }
            SidebarSettingsTarget::Appearance(target) => {
                self.layer.hover_card_for(target, clock)
            }
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.layer.close();
        }
    }

    pub fn update(&mut self, msg: SidebarSettingsMessage) -> Task<SidebarSettingsMessage> {
        match msg {
            SidebarSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) == ExpansionChange::JustCollapsed {
                    self.layer.close();
                }
                Task::none()
            }
            SidebarSettingsMessage::ToggleWorkspacesPlacement => {
                self.workspaces_placement = self.workspaces_placement.toggled();
                logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.workspaces_placement),
                )
            }
            SidebarSettingsMessage::ToggleEmptyWorkspaces => {
                self.empty_workspace_folding.toggle_temporary_reveal();
                self.reveal_fold_timer = None;
                Task::none()
            }
            SidebarSettingsMessage::ToggleAutoCollapseWorkspaces => {
                self.empty_workspace_folding.toggle_mode();
                self.reveal_fold_timer = None;
                logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.empty_workspace_folding.mode()),
                )
            }
            SidebarSettingsMessage::FoldTemporaryReveal => {
                self.empty_workspace_folding.fold_temporary_reveal();
                self.reveal_fold_timer = None;
                Task::none()
            }
            SidebarSettingsMessage::Appearance(msg) => self
                .layer
                .update(msg)
                .map(SidebarSettingsMessage::Appearance),
            SidebarSettingsMessage::PlacementLoaded(placement) => {
                self.workspaces_placement = placement;
                Task::none()
            }
            SidebarSettingsMessage::EmptyWorkspacesModeLoaded(empty_workspaces_mode) => {
                self.empty_workspace_folding
                    .adopt_mode(empty_workspaces_mode);
                self.reveal_fold_timer = None;
                Task::none()
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, SidebarSettingsMessage> {
        let sidebar_shown = self.workspaces_placement == WorkspacesPlacement::Sidebar;
        WidgetGroupContent::expandable(
            WidgetGroupFamily::SidebarSettings,
            &self.expansion,
            SidebarSettingsMessage::Expansion,
            [
                GroupItem::toggle(
                    workspaces_sidebar_toggle(self.workspaces_placement),
                    SidebarSettingsTarget::WorkspacesPlacement,
                    SidebarSettingsMessage::ToggleWorkspacesPlacement,
                ),
                GroupItem::toggle(
                    auto_collapse_workspaces_toggle(self.empty_workspace_folding.mode()),
                    SidebarSettingsTarget::AutoCollapseWorkspaces,
                    SidebarSettingsMessage::ToggleAutoCollapseWorkspaces,
                ),
            ]
            .into_iter()
            .chain(
                sidebar_shown
                    .then(|| {
                        self.layer
                            .items(config, SidebarSettingsMessage::Appearance)
                    })
                    .into_iter()
                    .flatten(),
            ),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::SidebarSimple(Weight::Regular).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SidebarSettingsGroup, SidebarSettingsMessage, WorkspacesPlacement};
    use crate::widget_groups::workspaces_group::EmptyWorkspaces;

    #[test]
    fn a_loaded_placement_reads_back_unchanged() {
        let mut sidebar_settings_group = SidebarSettingsGroup::default();
        let _ = sidebar_settings_group.update(SidebarSettingsMessage::PlacementLoaded(
            WorkspacesPlacement::BottomPanel,
        ));
        assert_eq!(
            sidebar_settings_group.workspaces_placement(),
            WorkspacesPlacement::BottomPanel
        );
    }

    #[test]
    fn revealing_empty_workspaces_leaves_auto_collapse_on() {
        let mut sidebar_settings_group = SidebarSettingsGroup::default();
        let _ = sidebar_settings_group.update(SidebarSettingsMessage::ToggleEmptyWorkspaces);
        assert_eq!(
            sidebar_settings_group.empty_workspace_folding().mode(),
            EmptyWorkspaces::Folded
        );
    }

    #[test]
    fn a_loaded_empty_workspaces_mode_reads_back_unchanged() {
        let mut sidebar_settings_group = SidebarSettingsGroup::default();
        let _ = sidebar_settings_group.update(SidebarSettingsMessage::EmptyWorkspacesModeLoaded(
            EmptyWorkspaces::Shown,
        ));
        assert_eq!(
            sidebar_settings_group.empty_workspace_folding().mode(),
            EmptyWorkspaces::Shown
        );
    }
}
