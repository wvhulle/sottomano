use iced::Task;
use sottomano_config::Config;
use sottomano_state::{
    LayerAppearance, ShellLayer, ShellLayout, WorkspacesPlacement, shell_settings,
};

use crate::{
    hover_tooltips::{GroupHandle, HoverTarget},
    shell_message::Message,
    widgets::{
        group_item::GroupItem,
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

fn workspaces_sidebar_toggle(shell_layout: &ShellLayout) -> IconToggle {
    IconToggle::new(
        "Workspaces sidebar",
        Phosphor::SidebarSimple(Weight::Regular),
        shell_layout.workspaces_placement == WorkspacesPlacement::Sidebar,
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
}

pub struct SidebarSettingsGroup {
    expansion: GroupExpansion,
    appearance: LayerAppearanceControls,
}

impl Default for SidebarSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            appearance: LayerAppearanceControls::new(ShellLayer::Sidebar, |target| {
                SidebarSettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl SidebarSettingsGroup {
    #[must_use]
    pub fn tooltip_for(
        &self,
        target: SidebarSettingsTarget,
        shell_layout: &ShellLayout,
        sidebar: &LayerAppearance,
    ) -> Tooltip {
        match target {
            SidebarSettingsTarget::WorkspacesPlacement => {
                workspaces_sidebar_toggle(shell_layout).switch_tooltip()
            }
            SidebarSettingsTarget::Appearance(target) => {
                self.appearance.tooltip_for(target, sidebar)
            }
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.appearance.close();
        }
    }

    pub fn adopt(&mut self, sidebar: &LayerAppearance) {
        self.appearance.adopt(sidebar);
    }

    pub fn update(
        &mut self,
        msg: SidebarSettingsMessage,
        shell_layout: &mut ShellLayout,
        sidebar: &mut LayerAppearance,
    ) -> Task<Message> {
        match msg {
            SidebarSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.appearance.close();
                }
                Task::none()
            }
            SidebarSettingsMessage::ToggleWorkspacesPlacement => {
                shell_layout.workspaces_placement = shell_layout.workspaces_placement.toggled();
                Task::future(shell_settings::save_workspaces_placement(
                    shell_layout.workspaces_placement,
                ))
                .discard()
            }
            SidebarSettingsMessage::Appearance(msg) => self
                .appearance
                .update(msg, sidebar)
                .map(|msg| Message::SidebarSettings(SidebarSettingsMessage::Appearance(msg))),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        shell_layout: &ShellLayout,
        sidebar: &LayerAppearance,
    ) -> GroupContent<'a, SidebarSettingsMessage> {
        let sidebar_shown = shell_layout.workspaces_placement == WorkspacesPlacement::Sidebar;
        GroupContent::new(
            &self.expansion,
            SidebarSettingsMessage::Expansion,
            [GroupItem::toggle(
                workspaces_sidebar_toggle(shell_layout),
                SidebarSettingsTarget::WorkspacesPlacement,
                SidebarSettingsMessage::ToggleWorkspacesPlacement,
            )]
            .into_iter()
            .chain(
                sidebar_shown
                    .then(|| {
                        self.appearance
                            .items(config, sidebar, SidebarSettingsMessage::Appearance)
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
