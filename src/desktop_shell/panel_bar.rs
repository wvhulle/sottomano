use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};

use crate::{
    desktop_shell::{DesktopShell, Message},
    widgets::region_reporter::{RegionRoles, region_reporter},
};

impl DesktopShell {
    pub(super) fn panel_bar<'a>(
        &'a self,
        start: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        center: impl IntoIterator<Item = Option<Element<'a, Message>>>,
        end: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let spacing = &self.config.spacing;
        let clusters = row![
            self.group_cluster(start),
            container(self.group_cluster(center)).center_x(Length::Fill),
            self.group_cluster(end),
        ]
        .spacing(spacing.group_spacing_px)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, spacing.group_padding_px]);
        self.bar_band(clusters)
    }

    fn bar_band<'a>(&'a self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let panels = &self.shell_settings.panels;
        let palette = &self.config.palette;
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !self.shell_settings.layout.disconnected_groups;
        let band_style = if groups_joined {
            self.shell_settings.frame_style(palette)
        } else {
            container::Style::default().color(palette.bar_foreground)
        };
        let strip = container(centered)
            .style(move |_| band_style)
            .width(Length::Fill)
            .height(Length::Fixed(self.config.bar.height_px));
        let roles = RegionRoles {
            takes_input: true,
            blurred: groups_joined,
            corner_radius: panels.corner_radius.top_left,
        };
        region_reporter(column![strip], roles).into()
    }

    fn group_cluster<'a>(
        &'a self,
        groups: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = &self.shell_settings.panels;
        let groups_disconnected = self.shell_settings.layout.disconnected_groups;
        let frame_style = self.shell_settings.frame_style(&config.palette);
        let framed = groups.into_iter().flatten().map(|group| {
            if !groups_disconnected {
                return group;
            }
            container(group)
                .style(move |_| frame_style)
                .height(Length::Fixed(config.bar.height_px))
                .align_y(Vertical::Center)
                .padding([
                    0.0,
                    panels.frame_corner_inset() + config.spacing.group_padding_px,
                ])
                .into()
        });
        let spacing = if groups_disconnected {
            config.spacing.widget_spacing_px
        } else {
            config.spacing.group_spacing_px
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
