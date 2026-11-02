use std::time::Duration;

use iced::{Task, task};
use sottomano_config::Config;
use sottomano_sources::desktop_windows_apps::niri_window_snapshot;
use tokio::time::sleep;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        sqlite_singleton_flag::{self, SingletonFlag},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
        workspaces_group::{EmptyWorkspaceFolding, EmptyWorkspaces, RevealFoldTimer},
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, ShellLayer},
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

async fn load_workspaces_placement() -> WorkspacesPlacement {
    if sqlite_singleton_flag::load(SingletonFlag::WorkspacesInSidebar, true).await {
        WorkspacesPlacement::Sidebar
    } else {
        WorkspacesPlacement::BottomPanel
    }
}

fn workspaces_sidebar_toggle(placement: WorkspacesPlacement) -> IconToggle {
    IconToggle::new(
        "Workspaces sidebar",
        Phosphor::SidebarSimple(Weight::Regular),
        placement == WorkspacesPlacement::Sidebar,
        SwitchPresses {
            turning_on: "move workspaces to a sidebar on the left edge",
            turning_off: "move workspaces inline into the bottom panel",
        },
    )
}

async fn load_empty_workspaces_mode() -> EmptyWorkspaces {
    if sqlite_singleton_flag::load(SingletonFlag::AutoCollapseWorkspaces, true).await {
        EmptyWorkspaces::Folded
    } else {
        EmptyWorkspaces::Shown
    }
}

fn auto_collapse_workspaces_toggle(empty_workspaces_mode: EmptyWorkspaces) -> IconToggle {
    IconToggle::new(
        "Auto-collapse unused workspaces",
        Phosphor::CaretUp(Weight::Regular),
        empty_workspaces_mode == EmptyWorkspaces::Folded,
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
    appearance: LayerAppearanceControls,
}

impl Default for SidebarSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: WidgetGroupExpansion::default(),
            workspaces_placement: WorkspacesPlacement::Sidebar,
            empty_workspace_folding: EmptyWorkspaceFolding::default(),
            reveal_fold_timer: None,
            appearance: LayerAppearanceControls::new(ShellLayer::Sidebar, |target| {
                SidebarSettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl SidebarSettingsGroup {
    pub fn load() -> Task<SidebarSettingsMessage> {
        Task::batch([
            Task::perform(
                load_workspaces_placement(),
                SidebarSettingsMessage::PlacementLoaded,
            ),
            Task::perform(
                load_empty_workspaces_mode(),
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
        niri: &niri_window_snapshot::Snapshot,
    ) -> Task<SidebarSettingsMessage> {
        match self.empty_workspace_folding.observe_workspace_switch(niri) {
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
    pub fn sidebar_appearance(&self) -> LayerAppearance {
        self.appearance.appearance()
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
                self.appearance.hover_card_for(target, clock)
            }
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.appearance.close();
        }
    }

    pub fn update(&mut self, msg: SidebarSettingsMessage) -> Task<SidebarSettingsMessage> {
        match msg {
            SidebarSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.appearance.close();
                }
                Task::none()
            }
            SidebarSettingsMessage::ToggleWorkspacesPlacement => {
                self.workspaces_placement = self.workspaces_placement.toggled();
                Task::future(sqlite_singleton_flag::save(
                    SingletonFlag::WorkspacesInSidebar,
                    self.workspaces_placement == WorkspacesPlacement::Sidebar,
                ))
                .discard()
            }
            SidebarSettingsMessage::ToggleEmptyWorkspaces => {
                self.empty_workspace_folding.toggle_temporary_reveal();
                self.reveal_fold_timer = None;
                Task::none()
            }
            SidebarSettingsMessage::ToggleAutoCollapseWorkspaces => {
                self.empty_workspace_folding.toggle_mode();
                self.reveal_fold_timer = None;
                Task::future(sqlite_singleton_flag::save(
                    SingletonFlag::AutoCollapseWorkspaces,
                    self.empty_workspace_folding.mode() == EmptyWorkspaces::Folded,
                ))
                .discard()
            }
            SidebarSettingsMessage::FoldTemporaryReveal => {
                self.empty_workspace_folding.fold_temporary_reveal();
                self.reveal_fold_timer = None;
                Task::none()
            }
            SidebarSettingsMessage::Appearance(msg) => self
                .appearance
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
                        self.appearance
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
