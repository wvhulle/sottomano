use iced::Task;
use sottomano_config::Config;
use sottomano_state::{
    LayerAppearance, ShellLayer, WorkspacesPlacement,
    shell_settings::{self, ShellSetting},
};

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        group_item::GroupItem,
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

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
    pub fn load() -> Task<SidebarSettingsMessage> {
        Task::batch([
            Task::perform(
                shell_settings::load_workspaces_placement(),
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
                Task::future(shell_settings::save(ShellSetting::WorkspacesPlacement(
                    self.workspaces_placement,
                )))
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
    use sottomano_state::WorkspacesPlacement;

    use super::{SidebarSettingsGroup, SidebarSettingsMessage};

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
