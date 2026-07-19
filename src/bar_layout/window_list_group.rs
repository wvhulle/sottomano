//! The niri window list (odin's `Windowlist` ConcreteWidget/WidgetGroup).
//! Stateless — always renders live niri state, so unlike battery/system it has
//! no owned fields.

use iced::{
    Alignment, Element,
    widget::{Row, image, mouse_area, text, text::Shaping},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::{app_icon::DesktopEntries, niri},
    layer_surface::Position,
    user_config::Config,
    visual_elements::{edge_slot, font_family, hairline},
};

/// The window list (odin's `Windowlist`): icon + title of the windows on the
/// output's active workspace, in on-screen order. Focus/urgency show as a
/// hairline along the inner edge (powerTint / criticalTint) — the same shared
/// indicator the workspace pager and the menu-group affordance use — not label
/// tinting; labels stay neutral. `None` when the workspace has no windows.
/// Clicking an entry focuses that window.
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
    let size = config.font.size as f32;
    let sans = font_family(&config.font.family);
    let entries = windows.into_iter().map(|w| {
        let mut parts: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = window_icon(config, desktop_entries, &w) {
            parts.push(icon);
        }
        parts.push(
            text(window_label(&w))
                .size(size)
                .font(sans)
                .shaping(Shaping::Advanced)
                .into(),
        );
        let content: Element<'a, Message> = Row::with_children(parts)
            .spacing(config.layout.horizontal_widget_spacing as f32 * 0.6)
            .align_y(Alignment::Center)
            .into();

        let (focus_color, focus_opacity) = if w.is_urgent {
            (config.palette.critical_tint.0, 1.0)
        } else if w.is_focused {
            (config.palette.power_tint.0, 1.0)
        } else {
            (config.palette.power_tint.0, 0.0)
        };
        let focus = hairline(
            focus_color,
            focus_opacity,
            config.layout.focus_underline_thickness,
        );

        // Clicking a window focuses it (odin's Windowlist `onActivated`).
        mouse_area(edge_slot(position, content, focus, None))
            .on_press(Message::FocusWindow(w.id))
            .into()
    });
    Some(
        Row::with_children(entries)
            .spacing(config.layout.horizontal_widget_spacing as f32 * 2.0)
            .align_y(Alignment::Center)
            .into(),
    )
}

/// A window's icon, resolved from its `app_id` via the installed `.desktop`
/// entries and the icon theme (odin's `WindowIcon._resolveAppId`). `None`
/// before the desktop-entry scan completes, or when nothing resolves (odin's
/// generic fallback icon still applies inside `icon_for`, so this is only
/// `None` on that final failure — kept as a graceful gap rather than a
/// broken-image placeholder).
fn window_icon<'a>(
    config: &Config,
    desktop_entries: Option<&DesktopEntries>,
    w: &niri::WinEntry,
) -> Option<Element<'a, Message>> {
    let path = desktop_entries?.icon_for(&w.app_id)?;
    let side = config.image_icon_size() as f32;
    Some(
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .into(),
    )
}

/// A window's label: its full title, else its app id. No truncation/ellipsis —
/// odin renders the whole title (`WindowTitle`), so we do too; the slot sizes to
/// the text.
fn window_label(w: &niri::WinEntry) -> String {
    if w.title.is_empty() {
        w.app_id.clone()
    } else {
        w.title.clone()
    }
}
