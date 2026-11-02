use std::time::Duration;

use iced::{Task, task};
use sottomano_config::Config;
use sottomano_sources::{desktop_windows_apps::niri_window_snapshot, sqlite_state_store::Columns};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        sqlite_shell_settings_row,
        sqlite_shell_settings_row::ShellSetting,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
        workspaces_group::{EmptyWorkspaceFolding, RevealFoldTimer},
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
    sqlite_shell_settings_row::load_stored_row(
        "shell settings: load workspaces placement failed",
        WorkspacesPlacement::Sidebar,
        workspaces_placement_from_columns,
    )
    .await
}

fn workspaces_placement_from_columns(
    columns: &Columns,
    row: &turso::Row,
    fallback: WorkspacesPlacement,
) -> WorkspacesPlacement {
    match columns.optional_flag(row, "workspaces_in_sidebar") {
        Some(true) => WorkspacesPlacement::Sidebar,
        Some(false) => WorkspacesPlacement::BottomPanel,
        None => fallback,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSettingsTarget {
    WorkspacesPlacement,
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
    FoldSteppingReveal,
    Appearance(AppearanceMessage),
    PlacementLoaded(WorkspacesPlacement),
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
                    .map(|()| SidebarSettingsMessage::FoldSteppingReveal)
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
                Task::future(sqlite_shell_settings_row::save(
                    ShellSetting::WorkspacesPlacement(self.workspaces_placement),
                ))
                .discard()
            }
            SidebarSettingsMessage::ToggleEmptyWorkspaces => {
                self.empty_workspace_folding.toggle_mode();
                self.reveal_fold_timer = None;
                Task::none()
            }
            SidebarSettingsMessage::FoldSteppingReveal => {
                self.empty_workspace_folding.fold_stepping_reveal();
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
            [GroupItem::toggle(
                workspaces_sidebar_toggle(self.workspaces_placement),
                SidebarSettingsTarget::WorkspacesPlacement,
                SidebarSettingsMessage::ToggleWorkspacesPlacement,
            )]
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
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{
        SidebarSettingsGroup, SidebarSettingsMessage, WorkspacesPlacement,
        workspaces_placement_from_columns,
    };
    use crate::widget_groups::sqlite_shell_settings_row::{
        CREATE_TABLE, ShellSetting, read_stored_row, write_setting,
    };

    async fn stored_placement(connection: &turso::Connection) -> WorkspacesPlacement {
        read_stored_row(
            connection,
            WorkspacesPlacement::Sidebar,
            workspaces_placement_from_columns,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_bottom_panel_placement_round_trips() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(
            &connection,
            ShellSetting::WorkspacesPlacement(WorkspacesPlacement::BottomPanel),
        )
        .await
        .unwrap();

        assert_eq!(
            stored_placement(&connection).await,
            WorkspacesPlacement::BottomPanel
        );
    }

    #[tokio::test]
    async fn an_unwritten_placement_falls_back_to_the_sidebar() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(&connection, ShellSetting::Sound(false))
            .await
            .unwrap();

        assert_eq!(
            stored_placement(&connection).await,
            WorkspacesPlacement::Sidebar
        );
    }

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
}
