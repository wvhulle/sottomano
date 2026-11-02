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
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    widget_groups::widget_group_content::WidgetGroupContent,
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

impl DesktopShell {
    pub(super) fn bar_group_menus(&self) -> impl Iterator<Item = GroupMenuView<'_, Message>> {
        [
            self.hardware_group_menu(),
            self.network_group_menu(),
            self.agenda_group_menu(),
            self.media_group_menu(),
            self.display_group_menu(),
            self.services_group_menu(),
            self.session_group_menu(),
        ]
        .into_iter()
        .chain(self.settings_menus())
    }

    pub(super) fn group_menu<'a>(
        &'a self,
        topic: GroupMenuTopic,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a, Message>>,
    ) -> GroupMenuView<'a, Message> {
        GroupMenuView::new(
            topic,
            &self.widget_groups.group_menus,
            Message::GroupMenuState,
            groups,
        )
    }

    pub(super) fn panel_bar<'a>(
        &'a self,
        position: Position,
        start: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        center: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        end: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let panels = &self.config.desktop_panels;
        let clusters = row![
            self.group_cluster(position, start),
            container(self.group_cluster(position, center)).center_x(Length::Fill),
            self.group_cluster(position, end),
        ]
        .spacing(panels.joined_group_gap)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, panels.group_edge_padding]);
        self.panel_band(position, clusters)
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
        let strip = panel_frame(band, LayerFrameStyle::new(&panels, &self.config))
            .outlined_sides(outlined_sides)
            .blurred(panels.blurred);
        let roles = RegionRoles {
            takes_input: true,
            ..RegionRoles::default()
        };
        region_reporter(column![strip], roles).into()
    }

    fn group_cluster<'a>(
        &'a self,
        position: Position,
        groups: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = self.widget_groups.panel_settings_group.panel_appearance();
        let groups_disconnected = self
            .widget_groups
            .panel_settings_group
            .groups_disconnected();
        let frame_style = LayerFrameStyle::new(&panels, config);
        let outlined_sides = OutlinedSides::inward_of(position)
            .all_when_floating(self.widget_groups.panel_settings_group.float_offset_px());
        let framed = groups.into_iter().flatten().map(|group| {
            if !groups_disconnected {
                return group;
            }
            panel_frame(
                container(group)
                    .height(Length::Fixed(config.desktop_panels.height))
                    .align_y(Vertical::Center)
                    .padding([
                        0.0,
                        panels.frame_corner_inset() + config.desktop_panels.group_edge_padding,
                    ]),
                frame_style,
            )
            .outlined_sides(outlined_sides)
            .blurred(panels.blurred)
            .into()
        });
        let spacing = if groups_disconnected {
            config.desktop_panels.framed_group_gap
        } else {
            config.desktop_panels.joined_group_gap
        };
        Row::with_children(framed)
            .spacing(spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    }
}
