use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    widgets::{
        panel_frame::{OutlinedSides, panel_frame},
        region_reporter::{RegionRoles, region_reporter},
    },
};

impl DesktopShell {
    pub(super) fn panel_bar<'a>(
        &'a self,
        position: Position,
        start: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        center: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        end: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let spacing = &self.config.panel_bar;
        let clusters = row![
            self.group_cluster(position, start),
            container(self.group_cluster(position, center)).center_x(Length::Fill),
            self.group_cluster(position, end),
        ]
        .spacing(spacing.group_spacing_px)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, spacing.group_padding_px]);
        self.bar_band(position, clusters)
    }

    fn bar_band<'a>(
        &'a self,
        position: Position,
        content: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let panels = &self.shell_settings.panels;
        let palette = &self.config.theme.palette;
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !self.shell_settings.layout.disconnected_groups;
        let band_style = if groups_joined {
            self.shell_settings.frame_style(&self.config)
        } else {
            container::Style::default().color(palette.bar_foreground)
        };
        let outlined_sides = self.shell_settings.outlined_sides(OutlinedSides {
            left: false,
            right: false,
            ..position.inward_sides()
        });
        let strip = panel_frame(
            container(centered)
                .width(Length::Fill)
                .height(Length::Fixed(self.config.panel_bar.height_px)),
            &band_style,
        )
        .outlined_sides(outlined_sides);
        let roles = RegionRoles {
            takes_input: true,
            blurred: groups_joined,
            corner_radius: panels.corner_radius.top_left,
        };
        region_reporter(column![strip], roles).into()
    }

    fn group_cluster<'a>(
        &'a self,
        position: Position,
        groups: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = &self.shell_settings.panels;
        let groups_disconnected = self.shell_settings.layout.disconnected_groups;
        let frame_style = self.shell_settings.frame_style(config);
        let outlined_sides = self.shell_settings.outlined_sides(position.inward_sides());
        let framed = groups.into_iter().flatten().map(|group| {
            if !groups_disconnected {
                return group;
            }
            panel_frame(
                container(group)
                    .height(Length::Fixed(config.panel_bar.height_px))
                    .align_y(Vertical::Center)
                    .padding([
                        0.0,
                        panels.frame_corner_inset() + config.panel_bar.group_padding_px,
                    ]),
                &frame_style,
            )
            .outlined_sides(outlined_sides)
            .into()
        });
        let spacing = if groups_disconnected {
            config.panel_bar.widget_spacing_px
        } else {
            config.panel_bar.group_spacing_px
        };
        let cluster = Row::with_children(framed)
            .spacing(spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center);
        let roles = RegionRoles {
            takes_input: false,
            blurred: groups_disconnected,
            corner_radius: panels.corner_radius.top_left,
        };
        region_reporter(cluster, roles).into()
    }
}
