use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    hover_tooltips::GroupHandle,
    widget_groups::group_menu_pins::GroupMenuPinsMessage,
    widgets::{
        panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
        region_reporter::{RegionRoles, region_reporter},
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

pub(super) fn toggle_group_menu_pin(group_menu: GroupHandle, group_title: &'static str) -> Message {
    Message::GroupMenuPins(GroupMenuPinsMessage::Toggle {
        group_menu,
        group_title,
    })
}

impl DesktopShell {
    pub(super) fn panel_bar<'a>(
        &'a self,
        position: Position,
        start: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        center: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        end: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let spacing = &self.config.layout.panel;
        let clusters = row![
            self.group_cluster(position, start),
            container(self.group_cluster(position, center)).center_x(Length::Fill),
            self.group_cluster(position, end),
        ]
        .spacing(spacing.group_spacing)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, spacing.group_padding]);
        self.bar_band(position, clusters)
    }

    fn bar_band<'a>(
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
            .height(Length::Fixed(self.config.layout.panel.height));
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
                    .height(Length::Fixed(config.layout.panel.height))
                    .align_y(Vertical::Center)
                    .padding([
                        0.0,
                        panels.frame_corner_inset() + config.layout.panel.group_padding,
                    ]),
                frame_style,
            )
            .outlined_sides(outlined_sides)
            .blurred(panels.blurred)
            .into()
        });
        let spacing = if groups_disconnected {
            config.layout.panel.widget_spacing
        } else {
            config.layout.panel.group_spacing
        };
        Row::with_children(framed)
            .spacing(spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    }
}
