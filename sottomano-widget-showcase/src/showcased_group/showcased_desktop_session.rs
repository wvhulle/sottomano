use std::collections::HashMap;

use iced::Element;
use iced_controller_diagram::ControllerArtwork;
use sottomano_compositor_model::{
    compositor_styling_model::AppWindowLayout, compositor_window_facts::WorkspaceName,
    compositor_window_state::CompositorWindowState,
};
use sottomano_desktop_sources::desktop_windows_apps::{
    configured_workspace_order::ConfiguredWorkspaces, desktop_entries::DesktopEntries,
    workspace_window_snapshot::Snapshot,
};
use sottomano_toml_config::{Config, screen_edges::Position};
use sottomano_widgets::{
    clock::Clock,
    desktop_panels::{panel_frame::inward_outlined_sides, panel_slot_drag::PanelSlotContext},
    hover_cards::{HoverTarget, hover_card_body::Section},
    icon_catalog::{
        drawable_icon_catalog::IconCatalog, favicons::Favicons, window_icons::WindowAssets,
    },
    indicator_groups::{
        window_group::{WindowGroup, WindowRowLayout, WindowTarget, WindowTitleLength},
        workspaces_group::{
            empty_workspace_folding::{EmptyWorkspaceFolding, EmptyWorkspaces},
            inline_strip, sidebar_column, workspace_hover_card,
        },
    },
    widgets::pointer_drag::{DragActivity, DragView},
};

use crate::{
    showcase_message::ShowcaseInteraction,
    showcase_program::SHOWCASE_FLOAT_OFFSET_PX,
    showcased_group::{SHOWCASE_OUTPUT, recorded_look},
};

fn configured_in_fixture_order(snapshot: &Snapshot) -> ConfiguredWorkspaces {
    snapshot
        .workspaces_for_output(SHOWCASE_OUTPUT)
        .iter()
        .filter_map(|indicator| match &indicator.name {
            WorkspaceName::Named(name) => Some(name.clone()),
            WorkspaceName::Unnamed => None,
        })
        .collect()
}

fn undragged_slot_context(
    config: &Config,
    edge: Position,
    clock: Clock,
    window_row_layout: WindowRowLayout,
) -> PanelSlotContext<'_> {
    let panel_appearance = recorded_look().panels;
    PanelSlotContext {
        config,
        edge,
        drag: DragView {
            drag: DragActivity::Idle,
            ghost_frame: panel_appearance.frame,
        },
        clock,
        panel_corner_radius_px: panel_appearance.frame.corner_radius_px,
        panel_float_offset_px: 0.0,
        compositor_window_gaps_px: 0.0,
        panel_frame_style: panel_appearance.frame,
        window_row_layout,
        window_title_length: WindowTitleLength::default(),
    }
}

pub struct ShowcasedDesktopSession {
    pub window_group: WindowGroup,
    pub window_row_layout: WindowRowLayout,
    pub compositor: CompositorWindowState,
    pub snapshot: Snapshot,
    pub icon_catalog: IconCatalog,
    pub desktop_entries: DesktopEntries,
    pub favicons: Favicons,
    pub controller_artwork: ControllerArtwork,
}

impl ShowcasedDesktopSession {
    pub(super) fn sidebar_groups<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<Element<'view, ShowcaseInteraction>> {
        sidebar_column::vertical_view(
            &self.snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(&self.snapshot)),
            &EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown),
            recorded_look().sidebar.frame.drawn_corners(
                inward_outlined_sides(Position::Left).all_when_floating(SHOWCASE_FLOAT_OFFSET_PX),
            ),
            undragged_slot_context(config, Position::Left, clock, self.window_row_layout),
        )
        .map(|workspaces| workspaces.map(ShowcaseInteraction::from))
        .into_iter()
        .collect()
    }

    pub(super) fn workspaces_and_windows<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<Element<'view, ShowcaseInteraction>> {
        let snapshot = &self.snapshot;
        let workspaces = inline_strip::inline_view(
            snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(snapshot)),
            &EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown),
            undragged_slot_context(config, Position::Bottom, clock, self.window_row_layout),
        )
        .map(|workspaces| workspaces.map(ShowcaseInteraction::from));
        let windows = self
            .window_group
            .view(
                snapshot,
                WindowAssets {
                    icon_catalog: &self.icon_catalog,
                    desktop_entries: &self.desktop_entries,
                    favicons: &self.favicons,
                    controller_artwork: &self.controller_artwork,
                },
                None,
                SHOWCASE_OUTPUT,
                undragged_slot_context(config, Position::Bottom, clock, self.window_row_layout),
            )
            .map(|element| element.map(ShowcaseInteraction::from));
        workspaces.into_iter().chain(windows).collect()
    }

    pub(super) fn standalone_hover_card(
        &self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Vec<Section> {
        let silent_windows = HashMap::new();
        let window_press_actions = |window: u64| {
            self.window_group
                .window_press_actions(&self.snapshot, &silent_windows, window, config)
        };
        match target {
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                WindowGroup::window_identity_hover_card(&self.compositor, *window, clock)
                    .into_iter()
                    .chain(Section::press_actions(window_press_actions(*window)))
                    .collect()
            }
            HoverTarget::Window(WindowTarget::Inspect(window)) => self
                .window_group
                .window_list
                .process_sections()
                .into_iter()
                .chain(Section::press_actions(window_press_actions(*window)))
                .collect(),
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.snapshot,
                &self.icon_catalog,
                AppWindowLayout::default(),
                *workspace,
            ),
            _ => Vec::new(),
        }
    }
}
