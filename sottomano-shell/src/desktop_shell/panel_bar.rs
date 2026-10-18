use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, column, container, mouse_area, row},
};
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    shell_message::Message,
    widgets::{
        palette_colors::PaletteColors,
        panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
        region_reporter::{RegionRoles, region_reporter},
    },
};

use crate::desktop_shell::DesktopShell;

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
        let panels = &self.shell_settings.panels;
        let palette = PaletteColors::from(&self.config.theme.palette);
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !self.shell_settings.layout.disconnected_groups;
        let band_style = if groups_joined {
            LayerFrameStyle::new(panels, &self.config).container_style()
        } else {
            container::Style::default().color(palette.bar_foreground)
        };
        let outlined_sides = OutlinedSides {
            left: false,
            right: false,
            ..OutlinedSides::inward_of(position)
        }
        .all_when_floating(&self.shell_settings.layout);
        let strip = panel_frame(
            container(centered)
                .width(Length::Fill)
                .height(Length::Fixed(self.config.layout.panel.height)),
            &band_style,
        )
        .outlined_sides(outlined_sides);
        let roles = RegionRoles {
            takes_input: true,
            blurred: panels.blurred && groups_joined,
            corner_radius: panels.corner_radius_px,
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
        let frame_style = LayerFrameStyle::new(panels, config).container_style();
        let outlined_sides =
            OutlinedSides::inward_of(position).all_when_floating(&self.shell_settings.layout);
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
                &frame_style,
            )
            .outlined_sides(outlined_sides)
            .into()
        });
        let spacing = if groups_disconnected {
            config.layout.panel.widget_spacing
        } else {
            config.layout.panel.group_spacing
        };
        let cluster = Row::with_children(framed)
            .spacing(spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center);
        let roles = RegionRoles {
            takes_input: false,
            blurred: panels.blurred && groups_disconnected,
            corner_radius: panels.corner_radius_px,
        };
        region_reporter(cluster, roles).into()
    }
}
