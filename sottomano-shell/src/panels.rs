pub(crate) mod bottom_panel;
pub(crate) mod panel_bar_view;
pub(crate) mod standalone_groups;
pub(crate) mod top_panel;
pub(crate) mod workspace_sidebar;

use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use self::{
    PanelZoneEntry::{Fixed, InlineWorkspaces, Menu, OutputWindows, Standalone},
    bottom_panel::BOTTOM_PANEL,
    top_panel::TOP_PANEL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelZone {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StandaloneWidgetGroup {
    Clock,
    Notifications,
    Tray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixedPanelEntry {
    SessionPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelZoneEntry {
    Menu(GroupMenuTopic),
    Standalone(StandaloneWidgetGroup),
    Fixed(FixedPanelEntry),
    InlineWorkspaces,
    OutputWindows,
}

pub(crate) struct PanelBarComposition {
    pub position: Position,
    pub(crate) start: &'static [PanelZoneEntry],
    pub(crate) center: &'static [PanelZoneEntry],
    pub(crate) end: &'static [PanelZoneEntry],
}

impl PanelBarComposition {
    pub(crate) const fn entries_in(&self, zone: PanelZone) -> &'static [PanelZoneEntry] {
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

fn all_panel_entries() -> impl Iterator<Item = PanelZoneEntry> {
    TOP_PANEL.entries().chain(BOTTOM_PANEL.entries())
}

pub(crate) fn composed_menu_topics() -> impl Iterator<Item = GroupMenuTopic> {
    all_panel_entries().filter_map(|entry| match entry {
        Menu(topic) => Some(topic),
        Standalone(_) | Fixed(_) | InlineWorkspaces | OutputWindows => None,
    })
}

pub(crate) fn composed_standalone_groups() -> impl Iterator<Item = StandaloneWidgetGroup> {
    all_panel_entries().filter_map(|entry| match entry {
        Standalone(group) => Some(group),
        Menu(_) | Fixed(_) | InlineWorkspaces | OutputWindows => None,
    })
}

#[cfg(test)]
mod tests {
    use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

    use super::{
        BOTTOM_PANEL, PanelZone,
        PanelZoneEntry::{Menu, Standalone},
        StandaloneWidgetGroup::{self, Clock, Notifications, Tray},
        TOP_PANEL, composed_menu_topics, composed_standalone_groups,
    };

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
        for group in [Clock, Notifications, Tray] {
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
        assert_eq!(
            BOTTOM_PANEL.entries_in(PanelZone::Start).first(),
            Some(&Menu(GroupMenuTopic::Applications))
        );
    }
}
