use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};

use crate::{
    app::{Bar, Message},
    config::Config,
    state::{PanelLayout, WorkspacesPlacement},
    wayland_layers::{
        bar::{corner_inset, pill_shadow},
        panel_regions::Zone,
    },
    widget_groups::workspaces_group,
    widgets::bounds_probe::bounds_probe,
};

pub fn view<'a>(bar: &'a Bar, window: Id, output: &str) -> Element<'a, Message> {
    let config = &bar.config;
    let layout = &bar.panel_settings.layout;
    let pill = (layout.workspaces_placement == WorkspacesPlacement::Sidebar)
        .then(|| workspaces_group::vertical_view(&bar.niri_snapshot, output, config))
        .flatten()
        .map(|workspaces| pill(workspaces.map(Message::from), config, layout));
    bounds_probe(Column::with_children(pill), move |rects| {
        Message::PillBounds {
            window,
            zone: Zone::Sidebar,
            rects,
        }
    })
}

fn pill<'a>(
    content: Element<'a, Message>,
    config: &Config,
    layout: &PanelLayout,
) -> Element<'a, Message> {
    let background = config.palette.surface.scale_alpha(layout.tint_factor);
    let foreground = config.palette.bar_foreground;
    let radius = layout.corner_radius;
    let shadow = pill_shadow(layout);
    container(content)
        .padding([corner_inset(radius) + config.spacing.group_padding_px, 0.0])
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius,
                ..Default::default()
            },
            shadow,
            text_color: Some(foreground),
            ..Default::default()
        })
        .into()
}
