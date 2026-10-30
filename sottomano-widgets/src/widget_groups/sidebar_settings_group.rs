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
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

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
    Expansion(ExpansionMessage),
    ToggleWorkspacesPlacement,
    Appearance(AppearanceMessage),
    PlacementLoaded(WorkspacesPlacement),
}

pub struct SidebarSettingsGroup {
    expansion: GroupExpansion,
    workspaces_placement: WorkspacesPlacement,
    appearance: LayerAppearanceControls,
}

impl Default for SidebarSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            workspaces_placement: WorkspacesPlacement::Sidebar,
            appearance: LayerAppearanceControls::new(ShellLayer::Sidebar, |target| {
                SidebarSettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl SidebarSettingsGroup {
    pub const NAME: &str = "Sidebar";

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
    pub fn sidebar_appearance(&self) -> LayerAppearance {
        self.appearance.appearance()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: SidebarSettingsTarget, clock: Clock) -> Tooltip {
        match target {
            SidebarSettingsTarget::WorkspacesPlacement => {
                workspaces_sidebar_toggle(self.workspaces_placement).switch_tooltip(clock)
            }
            SidebarSettingsTarget::Appearance(target) => self.appearance.tooltip_for(target, clock),
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
    ) -> GroupContent<'a, SidebarSettingsMessage> {
        let sidebar_shown = self.workspaces_placement == WorkspacesPlacement::Sidebar;
        GroupContent::new(
            Self::NAME,
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
        .face(HandleFace::Icon {
            glyph: Phosphor::SidebarSimple(Weight::Regular).into(),
            handle: GroupHandle::SidebarSettings,
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
