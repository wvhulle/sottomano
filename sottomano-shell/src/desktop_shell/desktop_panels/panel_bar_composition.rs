use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use self::{
    FixedPanelEntry::SessionPower,
    PanelZoneEntry::{Fixed, InlineWorkspaces, Menu, OutputWindows, Standalone},
    StandaloneWidgetGroup::{Clock, GlobalMenu, Notifications, Tray},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::desktop_shell) enum PanelZone {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::desktop_shell) enum StandaloneWidgetGroup {
    Clock,
    Notifications,
    Tray,
    GlobalMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::desktop_shell) enum FixedPanelEntry {
    SessionPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::desktop_shell) enum PanelZoneEntry {
    Menu(GroupMenuTopic),
    Standalone(StandaloneWidgetGroup),
    Fixed(FixedPanelEntry),
    InlineWorkspaces,
    OutputWindows,
}

pub(in crate::desktop_shell) struct PanelBarComposition {
    pub position: Position,
    start: &'static [PanelZoneEntry],
    center: &'static [PanelZoneEntry],
    end: &'static [PanelZoneEntry],
}

impl PanelBarComposition {
    pub(in crate::desktop_shell) const fn entries_in(
        &self,
        zone: PanelZone,
    ) -> &'static [PanelZoneEntry] {
        match zone {
            PanelZone::Start => self.start,
            PanelZone::Center => self.center,
            PanelZone::End => self.end,
        }
    }

    fn entries(&self) -> impl Iterator<Item = PanelZoneEntry> {
        [self.start, self.center, self.end]
            .into_iter()
            .flatten()
            .copied()
    }
}

pub(in crate::desktop_shell) const TOP_PANEL: PanelBarComposition = PanelBarComposition {
    position: Position::Top,
    start: &[
        Menu(GroupMenuTopic::Hardware),
        Menu(GroupMenuTopic::Network),
        Standalone(GlobalMenu),
    ],
    center: &[
        Standalone(Clock),
        Menu(GroupMenuTopic::Agenda),
        Standalone(Notifications),
    ],
    end: &[
        Menu(GroupMenuTopic::Display),
        Menu(GroupMenuTopic::Media),
        Fixed(SessionPower),
    ],
};

pub(in crate::desktop_shell) const BOTTOM_PANEL: PanelBarComposition = PanelBarComposition {
    position: Position::Bottom,
    start: &[
        Menu(GroupMenuTopic::Appearance),
        Menu(GroupMenuTopic::Behaviour),
        InlineWorkspaces,
    ],
    center: &[Menu(GroupMenuTopic::Applications), OutputWindows],
    end: &[
        Menu(GroupMenuTopic::Services),
        Menu(GroupMenuTopic::Session),
        Standalone(Tray),
    ],
};

fn all_panel_entries() -> impl Iterator<Item = PanelZoneEntry> {
    TOP_PANEL.entries().chain(BOTTOM_PANEL.entries())
}

pub(in crate::desktop_shell) fn composed_menu_topics() -> impl Iterator<Item = GroupMenuTopic> {
    all_panel_entries().filter_map(|entry| match entry {
        Menu(topic) => Some(topic),
        Standalone(_) | Fixed(_) | InlineWorkspaces | OutputWindows => None,
    })
}

pub(in crate::desktop_shell) fn composed_standalone_groups()
-> impl Iterator<Item = StandaloneWidgetGroup> {
    all_panel_entries().filter_map(|entry| match entry {
        Standalone(group) => Some(group),
        Menu(_) | Fixed(_) | InlineWorkspaces | OutputWindows => None,
    })
}

#[cfg(test)]
mod tests {
    use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

    use super::{
        BOTTOM_PANEL, Clock, GlobalMenu, Notifications, PanelZone, StandaloneWidgetGroup,
        TOP_PANEL, Tray, composed_menu_topics, composed_standalone_groups,
    };
    use crate::desktop_shell::desktop_panels::panel_bar_composition::{Menu, Standalone};

    #[test]
    fn every_menu_topic_sits_on_exactly_one_panel() {
        let composed: Vec<GroupMenuTopic> = composed_menu_topics().collect();
        for topic in GroupMenuTopic::ALL {
            let placements = composed.iter().filter(|placed| **placed == topic).count();
            assert_eq!(placements, 1, "{}", topic.title());
        }
    }

    #[test]
    fn standalone_groups_appear_once() {
        let composed: Vec<StandaloneWidgetGroup> = composed_standalone_groups().collect();
        for group in [Clock, Notifications, Tray, GlobalMenu] {
            assert_eq!(
                composed.iter().filter(|placed| **placed == group).count(),
                1
            );
        }
    }

    #[test]
    fn zones_keep_their_declared_order() {
        assert_eq!(
            TOP_PANEL.entries_in(PanelZone::Center),
            [
                Standalone(Clock),
                Menu(GroupMenuTopic::Agenda),
                Standalone(Notifications),
            ]
        );
        assert_eq!(
            BOTTOM_PANEL.entries_in(PanelZone::End).last(),
            Some(&Standalone(Tray))
        );
    }
}
