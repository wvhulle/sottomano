//! Niri window list.

use iced::{
    Alignment, Element,
    widget::{Row, image, mouse_area},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::{app_icon::DesktopEntries, niri},
    layer_surface::Position,
    user_config::Config,
    widgets::{activity_line, bar_text, pager_slot},
};

pub fn view<'a>(
    niri: &EventStreamState,
    output: &str,
    config: &Config,
    desktop_entries: Option<&DesktopEntries>,
    position: Position,
) -> Option<Element<'a, Message>> {
    let workspace = niri::active_workspace_id(niri, output)?;
    let windows = niri::windows_on_workspace(niri, workspace);
    if windows.is_empty() {
        return None;
    }
    let entries = windows.into_iter().map(|w| {
        let mut parts: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = window_icon(config, desktop_entries, &w) {
            parts.push(icon);
        }
        parts.push(bar_text(window_label(&w)).into());
        let content: Element<'a, Message> = Row::with_children(parts)
            .spacing(config.layout.horizontal_widget_spacing * 0.6)
            .align_y(Alignment::Center)
            .into();

        let (focus_color, focus_opacity) = if w.is_urgent {
            (config.palette.urgent(), 1.0)
        } else if w.is_focused {
            (config.palette.focus(), 1.0)
        } else {
            (config.palette.focus(), 0.0)
        };
        let focus = activity_line(
            focus_color,
            focus_opacity,
            config.layout.focus_underline_thickness,
        );

        mouse_area(pager_slot(position, content, focus, None))
            .on_press(Message::FocusWindow(w.id))
            .into()
    });
    Some(
        Row::with_children(entries)
            .spacing(config.layout.horizontal_widget_spacing * 2.0)
            .align_y(Alignment::Center)
            .into(),
    )
}

fn window_icon<'a>(
    config: &Config,
    desktop_entries: Option<&DesktopEntries>,
    w: &niri::WinEntry,
) -> Option<Element<'a, Message>> {
    let path = desktop_entries?.icon_for(&w.app_id)?;
    let side = config.font.icon_size;
    Some(
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .into(),
    )
}

fn window_label(w: &niri::WinEntry) -> String {
    if w.title.is_empty() {
        w.app_id.clone()
    } else {
        w.title.clone()
    }
}
