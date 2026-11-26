use std::collections::HashMap;

use sottomano_widgets::{
    group_indicators::group_indicator::{
        GroupIndicator, HoverFetch, IndicatorRefresh, RowHoverCard,
    },
    hover_cards::{
        CardHeader, HoverTarget, PressAction,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    output_windows::{OutputWindows, WindowTarget},
    workspace_switcher::{workspace_hover_card, workspace_order_press_actions},
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{PanelZoneEntry, all_panel_entries},
};

pub struct HoveredRowRequests {
    pub hover_fetch: HoverFetch,
    pub refresh: IndicatorRefresh,
}

impl From<&GroupIndicator<'_>> for HoveredRowRequests {
    fn from(hovered_row: &GroupIndicator<'_>) -> Self {
        Self {
            hover_fetch: hovered_row.hover_fetch.clone(),
            refresh: hovered_row.refresh.clone(),
        }
    }
}

impl DesktopShell {
    fn indicator_hover_card(&self, target: &HoverTarget) -> Option<RowHoverCard> {
        all_panel_entries().find_map(|entry| match entry {
            PanelZoneEntry::Menu(topic) => {
                self.topic_group_menu(topic).hover_card(target, self.clock)
            }
            PanelZoneEntry::InlineWorkspaces | PanelZoneEntry::OutputWindows => None,
        })
    }

    #[must_use]
    pub(crate) fn hovered_indicator_requests(
        &self,
        target: &HoverTarget,
    ) -> Option<HoveredRowRequests> {
        all_panel_entries().find_map(|entry| match entry {
            PanelZoneEntry::Menu(topic) => self
                .topic_group_menu(topic)
                .hovered_indicator(target)
                .map(HoveredRowRequests::from),
            PanelZoneEntry::InlineWorkspaces | PanelZoneEntry::OutputWindows => None,
        })
    }

    #[must_use]
    pub fn described_hover_card(&self, target: &HoverTarget) -> Option<HoverCard> {
        match self.indicator_hover_card(target) {
            Some(indicator_card) => HoverCard::new(indicator_card.sections, indicator_card.header),
            None => HoverCard::new(self.standalone_hover_card(target), CardHeader::Unheaded),
        }
    }

    fn window_press_actions(&self, window: u64) -> Vec<PressAction> {
        let silent_windows = HashMap::new();
        self.indicator_groups.output_windows.window_press_actions(
            &self.window_snapshot,
            self.indicator_groups
                .audio_group
                .window_audio()
                .unwrap_or(&silent_windows),
            window,
            &self.config,
        )
    }

    fn standalone_hover_card(&self, target: &HoverTarget) -> Vec<Section> {
        let output_windows = &self.indicator_groups.output_windows;
        match target {
            HoverTarget::Window(WindowTarget::Inspect(window)) => output_windows
                .window_list
                .process_sections()
                .into_iter()
                .chain(Section::press_actions(self.window_press_actions(*window)))
                .collect(),
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                OutputWindows::window_identity_hover_card(&self.compositor, *window, self.clock)
                    .into_iter()
                    .chain(
                        self.indicator_groups
                            .session_focus_group
                            .window_usage_hover_card(*window, &self.config, self.clock),
                    )
                    .chain(Section::press_actions(self.window_press_actions(*window)))
                    .collect()
            }
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.window_snapshot,
                &self.icon_catalog,
                self.indicator_groups
                    .compositor_styling_group
                    .app_window_layout(),
                *workspace,
            ),
            HoverTarget::WorkspaceOrder => Section::press_actions(workspace_order_press_actions())
                .into_iter()
                .collect(),
            HoverTarget::WorkspaceFolding => Section::press_actions(
                self.indicator_groups
                    .sidebar_settings_group
                    .empty_workspace_folding
                    .press_actions(),
            )
            .into_iter()
            .collect(),
            HoverTarget::Indicator(_)
            | HoverTarget::PinnedOnPanel(_)
            | HoverTarget::GroupHeadline(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::WindowTopMenu(_) => Vec::new(),
        }
    }
}
