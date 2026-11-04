use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    desktop_panels::{
        panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
        region_reporter::{RegionRoles, region_reporter},
    },
    hover_cards::HoverTarget,
    widget_groups::{sidebar_settings_group::WorkspacesPlacement, workspaces_group::inline_strip},
};

use crate::{
    desktop_shell::{
        DesktopShell,
        desktop_panels::panel_bar_composition::{PanelBarComposition, PanelZone, PanelZoneEntry},
    },
    shell_message::Message,
};

impl DesktopShell {
    pub(in crate::desktop_shell) fn panel_bar_view<'a>(
        &'a self,
        composition: &PanelBarComposition,
        output: &'a str,
    ) -> Element<'a, Message> {
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
    ) -> Vec<Element<'a, Message>> {
        let config = &self.config;
        match entry {
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
        content: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let panels = self.widget_groups.panel_settings_group.panel_appearance();
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !self
            .widget_groups
            .panel_settings_group
            .groups_disconnected();
        let band = container(centered)
            .width(Length::Fill)
            .height(Length::Fixed(self.config.desktop_panels.height));
        if !groups_joined {
            let roles = RegionRoles {
                takes_input: true,
                ..RegionRoles::default()
            };
            return region_reporter(column![band], roles).into();
        }
        let outlined_sides = OutlinedSides {
            left: false,
            right: false,
            ..OutlinedSides::inward_of(position)
        }
        .all_when_floating(self.widget_groups.panel_settings_group.float_offset_px());
        let strip = panel_frame(band, LayerFrameStyle::new(&panels))
            .outlined_sides(outlined_sides)
            .blurred(panels.blurred);
        let roles = RegionRoles {
            takes_input: true,
            ..RegionRoles::default()
        };
        region_reporter(column![strip], roles).into()
    }

    fn panel_zone_row<'a>(
        &'a self,
        position: Position,
        groups: impl IntoIterator<Item = Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = self.widget_groups.panel_settings_group.panel_appearance();
        let groups_disconnected = self
            .widget_groups
            .panel_settings_group
            .groups_disconnected();
        let frame_style = LayerFrameStyle::new(&panels);
        let outlined_sides = OutlinedSides::inward_of(position)
            .all_when_floating(self.widget_groups.panel_settings_group.float_offset_px());
        let framed = groups.into_iter().map(|group| {
            if !groups_disconnected {
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
            .blurred(panels.blurred)
            .into()
        });
        let spacing = if groups_disconnected {
            config.theme.spacing.item_gap
        } else {
            config.theme.spacing.group_gap
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
    ) -> Option<Element<'a, Message>> {
        match self
            .widget_groups
            .sidebar_settings_group
            .workspaces_placement()
        {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => inline_strip::inline_view(
                &self.niri_snapshot,
                output,
                self.configured_workspaces.as_ref(),
                self.widget_groups
                    .sidebar_settings_group
                    .empty_workspace_folding(),
                self.panel_slot_context(position),
            )
            .map(|workspaces| workspaces.map(Message::from)),
        }
    }

    fn window_group_view<'a>(
        &'a self,
        position: Position,
        output: &'a str,
    ) -> Option<Element<'a, Message>> {
        self.widget_groups
            .window_group
            .view(
                &self.niri_snapshot,
                self.window_assets(),
                self.widget_groups.audio_group.window_audio(),
                output,
                self.panel_slot_context(position),
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))))
    }
}
