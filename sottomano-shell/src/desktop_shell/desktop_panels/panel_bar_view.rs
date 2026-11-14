use corredo::theming::OutlinedSides;
use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    desktop_panels::{
        panel_frame::{outlined_sides_inward_of, panel_frame},
        region_reporter::{PointerInput, RegionRoles, region_reporter},
    },
    group_menus::group_menu_state::GroupMenuTopic,
    hover_cards::HoverTarget,
    widget_groups::{
        panel_settings_group::PanelGroupFraming, sidebar_settings_group::WorkspacesPlacement,
        workspaces_group::inline_strip,
    },
};

use crate::{
    desktop_shell::{
        DesktopShell,
        desktop_panels::panel_bar_composition::{PanelBarComposition, PanelZone, PanelZoneEntry},
    },
    shell_message::ShellInteraction,
};

impl DesktopShell {
    pub(in crate::desktop_shell) fn panel_bar_view<'a>(
        &'a self,
        composition: &PanelBarComposition,
        output: &'a str,
    ) -> Element<'a, ShellInteraction> {
        let position = composition.position;
        let zone_row = |zone| {
            self.panel_zone_row(
                position,
                composition
                    .entries_in(zone)
                    .iter()
                    .flat_map(|entry| self.panel_entry_views(*entry, position, output)),
            )
        };
        let panel_zone_rows = row![
            zone_row(PanelZone::Start),
            container(zone_row(PanelZone::Center)).center_x(Length::Fill),
            zone_row(PanelZone::End),
        ]
        .spacing(self.config.theme.spacing.group_gap)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, self.config.theme.spacing.item_gap]);
        self.panel_band(position, panel_zone_rows)
    }

    fn panel_entry_views<'a>(
        &'a self,
        entry: PanelZoneEntry,
        position: Position,
        output: &'a str,
    ) -> Vec<Element<'a, ShellInteraction>> {
        let config = &self.config;
        match entry {
            PanelZoneEntry::Menu(GroupMenuTopic::ApplicationMenu)
                if !self.widget_groups.global_menu_group.shows_top_menus() =>
            {
                Vec::new()
            }
            PanelZoneEntry::Menu(topic) => self
                .group_menu_of(topic, output)
                .panel_views(
                    config,
                    position,
                    self.hover_card
                        .handle_indicator(&HoverTarget::GroupMenu(topic), output),
                    self.clock,
                )
                .collect(),
            PanelZoneEntry::Standalone(group) => self
                .standalone_group(group)
                .view(config, position, self.clock)
                .into_iter()
                .collect(),
            PanelZoneEntry::Fixed(entry) => {
                let fixed_group = self.fixed_panel_group(entry);
                let handle_indicator = fixed_group
                    .handle_target()
                    .map(|target| self.hover_card.handle_indicator(target, output))
                    .unwrap_or_default();
                fixed_group
                    .view(
                        config,
                        position,
                        self.widget_groups.group_menus.presentation().open_trigger,
                        handle_indicator,
                        self.clock,
                    )
                    .into_iter()
                    .collect()
            }
            PanelZoneEntry::InlineWorkspaces => self
                .inline_workspaces(position, output)
                .into_iter()
                .collect(),
            PanelZoneEntry::OutputWindows => self
                .window_group_view(position, output)
                .into_iter()
                .collect(),
        }
    }

    fn panel_band<'a>(
        &'a self,
        position: Position,
        content: impl Into<Element<'a, ShellInteraction>>,
    ) -> Element<'a, ShellInteraction> {
        let panels = self.widget_groups.panel_settings_group.layer.appearance();
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(ShellInteraction::DismissPressed);
        let group_framing = self.widget_groups.panel_settings_group.group_framing();
        let band = container(centered)
            .width(Length::Fill)
            .height(Length::Fixed(self.config.desktop_panels.height));
        if group_framing == PanelGroupFraming::Disconnected {
            let roles = RegionRoles {
                pointer_input: PointerInput::Accepted,
                ..RegionRoles::default()
            };
            return region_reporter(column![band], roles).into();
        }
        let outlined_sides = OutlinedSides {
            left: false,
            right: false,
            ..outlined_sides_inward_of(position)
        }
        .all_when_floating(self.widget_groups.panel_settings_group.float_offset_px());
        let strip = panel_frame(band, panels.frame_style())
            .outlined_sides(outlined_sides)
            .backdrop_blur(panels.backdrop_blur);
        let roles = RegionRoles {
            pointer_input: PointerInput::Accepted,
            ..RegionRoles::default()
        };
        region_reporter(column![strip], roles).into()
    }

    fn panel_zone_row<'a>(
        &'a self,
        position: Position,
        groups: impl IntoIterator<Item = Element<'a, ShellInteraction>>,
    ) -> Element<'a, ShellInteraction> {
        let config = &self.config;
        let panels = self.widget_groups.panel_settings_group.layer.appearance();
        let group_framing = self.widget_groups.panel_settings_group.group_framing();
        let frame_style = panels.frame_style();
        let outlined_sides = outlined_sides_inward_of(position)
            .all_when_floating(self.widget_groups.panel_settings_group.float_offset_px());
        let framed = groups.into_iter().map(|group| {
            if group_framing == PanelGroupFraming::Joined {
                return group;
            }
            panel_frame(
                container(container(group).padding([0.0, config.theme.spacing.item_gap]))
                    .height(Length::Fixed(config.desktop_panels.height))
                    .align_y(Vertical::Center)
                    .padding([0.0, panels.frame_corner_inset()]),
                frame_style,
            )
            .outlined_sides(outlined_sides)
            .backdrop_blur(panels.backdrop_blur)
            .into()
        });
        let spacing = match group_framing {
            PanelGroupFraming::Disconnected => config.theme.spacing.item_gap,
            PanelGroupFraming::Joined => config.theme.spacing.group_gap,
        };
        Row::with_children(framed)
            .spacing(spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    }

    fn inline_workspaces<'a>(
        &'a self,
        position: Position,
        output: &'a str,
    ) -> Option<Element<'a, ShellInteraction>> {
        match self
            .widget_groups
            .sidebar_settings_group
            .workspaces_placement()
        {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => inline_strip::inline_view(
                &self.window_snapshot,
                output,
                self.configured_workspaces.as_ref(),
                self.widget_groups
                    .sidebar_settings_group
                    .empty_workspace_folding(),
                self.panel_slot_context(position),
            )
            .map(|workspaces| workspaces.map(ShellInteraction::from)),
        }
    }

    fn window_group_view<'a>(
        &'a self,
        position: Position,
        output: &'a str,
    ) -> Option<Element<'a, ShellInteraction>> {
        self.widget_groups
            .window_group
            .view(
                &self.window_snapshot,
                self.window_assets(),
                self.widget_groups.audio_group.window_audio(),
                output,
                self.panel_slot_context(position),
            )
            .map(|element| {
                element
                    .map(|event| ShellInteraction::from(event.map(ShellInteraction::WindowGroup)))
            })
    }
}
