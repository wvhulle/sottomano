use corredo::theming::{OutlineTracing, OutlinedSides};
use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    desktop_panels::{
        panel_frame::{inward_outlined_sides, panel_frame},
        region_reporter::{PointerInput, RegionRoles, region_reporter},
    },
    group_menus::group_menu_state::GroupMenuTopic,
    hover_cards::HoverTarget,
    panel_menus::{
        appearance_menu::{
            panel_settings_group::PanelGroupFraming, sidebar_settings_group::WorkspacesPlacement,
        },
        session_power_menu::session_power_group::PowerMenuPresence,
    },
    workspace_switcher::inline_strip,
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{PanelBarComposition, PanelZone, PanelZoneEntry},
    shell_message::ShellInteraction,
};

impl DesktopShell {
    pub(crate) fn panel_bar_view<'view>(
        &'view self,
        composition: &PanelBarComposition,
        output: &'view str,
    ) -> Element<'view, ShellInteraction> {
        let position = composition.position;
        let zone_indicator = |zone| {
            self.panel_zone_row(
                position,
                composition
                    .entries_in(zone)
                    .iter()
                    .flat_map(|entry| self.panel_entry_views(*entry, position, output)),
            )
        };
        let panel_zone_rows = row![
            zone_indicator(PanelZone::Start),
            container(zone_indicator(PanelZone::Center)).center_x(Length::Fill),
            zone_indicator(PanelZone::End),
        ]
        .spacing(self.config.theme.spacing.group_gap)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, self.config.theme.spacing.item_gap]);
        self.panel_band(position, panel_zone_rows)
    }

    fn panel_entry_views<'view>(
        &'view self,
        entry: PanelZoneEntry,
        position: Position,
        output: &'view str,
    ) -> Vec<Element<'view, ShellInteraction>> {
        let config = &self.config;
        match entry {
            PanelZoneEntry::Menu(GroupMenuTopic::WindowMenuBar)
                if !self.indicator_groups.window_menu_bar.shows_top_menus() =>
            {
                Vec::new()
            }
            PanelZoneEntry::Menu(GroupMenuTopic::WindowMenuBar)
                if self
                    .indicator_groups
                    .window_menu_bar
                    .shows_inline_top_menus() =>
            {
                self.indicator_groups
                    .window_menu_bar
                    .inline_handle_views(
                        config,
                        position,
                        self.indicator_groups.group_menus.presentation.open_trigger,
                        |target| self.hover_card.handle_indicator(target, output),
                    )
                    .into_iter()
                    .map(|element| element.map(ShellInteraction::from))
                    .collect()
            }
            PanelZoneEntry::Menu(GroupMenuTopic::SessionPower)
                if self.indicator_groups.session_power_group.menu_presence()
                    == PowerMenuPresence::Hidden =>
            {
                Vec::new()
            }
            PanelZoneEntry::Menu(topic) => self
                .topic_group_menu(topic)
                .panel_views(
                    config,
                    position,
                    self.hover_card
                        .handle_indicator(&HoverTarget::GroupMenu(topic), output),
                    self.clock,
                )
                .map(|element| element.map(ShellInteraction::from))
                .collect(),
            PanelZoneEntry::InlineWorkspaces => self
                .inline_workspaces(position, output)
                .into_iter()
                .collect(),
            PanelZoneEntry::OutputWindows => self
                .output_windows_view(position, output)
                .into_iter()
                .collect(),
        }
    }

    fn panel_band<'view>(
        &'view self,
        position: Position,
        content: impl Into<Element<'view, ShellInteraction>>,
    ) -> Element<'view, ShellInteraction> {
        let panels = self
            .indicator_groups
            .panel_settings_group
            .layer
            .appearance();
        let band = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fixed(self.config.desktop_panels.height))
                .align_y(Vertical::Center),
        )
        .on_press(ShellInteraction::DismissPressed);
        let group_framing = self.indicator_groups.panel_settings_group.group_framing;
        if group_framing == PanelGroupFraming::Disconnected {
            let roles = RegionRoles {
                pointer_input: PointerInput::Accepted,
                ..RegionRoles::default()
            };
            return region_reporter(column![band], roles).into();
        }
        let outlined_sides = OutlinedSides {
            left: OutlineTracing::Omitted,
            right: OutlineTracing::Omitted,
            ..inward_outlined_sides(position)
        }
        .all_when_floating(self.indicator_groups.panel_settings_group.float_offset_px());
        let strip = panel_frame(band, panels.frame)
            .outlined_sides(outlined_sides)
            .backdrop_blur(panels.backdrop_blur);
        let roles = RegionRoles {
            pointer_input: PointerInput::Accepted,
            ..RegionRoles::default()
        };
        region_reporter(column![strip], roles).into()
    }

    fn panel_zone_row<'view>(
        &'view self,
        position: Position,
        groups: impl IntoIterator<Item = Element<'view, ShellInteraction>>,
    ) -> Element<'view, ShellInteraction> {
        let config = &self.config;
        let panels = self
            .indicator_groups
            .panel_settings_group
            .layer
            .appearance();
        let group_framing = self.indicator_groups.panel_settings_group.group_framing;
        let frame_style = panels.frame;
        let outlined_sides = inward_outlined_sides(position)
            .all_when_floating(self.indicator_groups.panel_settings_group.float_offset_px());
        let framed = groups.into_iter().map(|group| {
            if group_framing == PanelGroupFraming::Joined {
                return group;
            }
            panel_frame(
                container(group)
                    .height(Length::Fixed(config.desktop_panels.height))
                    .align_y(Vertical::Center)
                    .padding([
                        0.0,
                        config.theme.spacing.item_gap + panels.frame_corner_inset(),
                    ]),
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

    fn inline_workspaces<'view>(
        &'view self,
        position: Position,
        output: &'view str,
    ) -> Option<Element<'view, ShellInteraction>> {
        match self
            .indicator_groups
            .sidebar_settings_group
            .workspaces_placement
        {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => inline_strip::inline_view(
                &self.window_snapshot,
                output,
                self.configured_workspaces.read_order(),
                &self
                    .indicator_groups
                    .sidebar_settings_group
                    .empty_workspace_folding,
                self.panel_slot_context(position),
            )
            .map(|workspaces| workspaces.map(ShellInteraction::from)),
        }
    }

    fn output_windows_view<'view>(
        &'view self,
        position: Position,
        output: &'view str,
    ) -> Option<Element<'view, ShellInteraction>> {
        self.indicator_groups
            .output_windows
            .view(
                &self.window_snapshot,
                self.window_assets(),
                self.indicator_groups.audio_group.window_audio(),
                output,
                self.panel_slot_context(position),
            )
            .map(|element| element.map(ShellInteraction::from))
    }
}
