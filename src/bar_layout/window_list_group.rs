//! The niri window list (odin's `Windowlist` ConcreteWidget/WidgetGroup). Stateless —
//! always renders live niri state, so unlike battery/system it has no owned fields.

use iced::widget::text::Shaping;
use iced::widget::{Row, image, mouse_area, text};
use iced::{Alignment, Color, Element};
use niri_ipc::state::EventStreamState;

use crate::bar_program::Message;
use crate::data_sources::app_icon::DesktopEntries;
use crate::data_sources::niri;
use crate::user_config::Config;
use crate::visual_elements::{font_family, state_pill};

/// The window list (odin's `Windowlist`): icon + title of the windows on the output's
/// active workspace, in on-screen order. The focused entry carries a pill background
/// (powerTint), an urgent one criticalTint — the container's own style, not a separate
/// sized indicator element (see `state_pill`'s doc comment for why). `None` when the
/// workspace has no windows. Clicking an entry focuses that window.
pub fn view<'a>(
    niri: &EventStreamState,
    output: &str,
    config: &Config,
    desktop_entries: Option<&DesktopEntries>,
) -> Option<Element<'a, Message>> {
    let workspace = niri::active_workspace_id(niri, output)?;
    let windows = niri::windows_on_workspace(niri, workspace);
    if windows.is_empty() {
        return None;
    }
    eprintln!(
        "window_list: output={} focus={:?}",
        output,
        windows.iter().map(|w| (w.id, w.is_focused)).collect::<Vec<_>>()
    );
    let size = config.font.size as f32;
    let sans = font_family(&config.font.family);
    let entries = windows.into_iter().map(|w| {
        let label_color = if w.is_focused || w.is_urgent {
            Color::WHITE
        } else {
            config.palette.muted.0
        };
        let mut parts: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = window_icon(config, desktop_entries, &w) {
            parts.push(icon);
        }
        parts.push(
            text(window_label(&w)).size(size).font(sans).color(label_color).shaping(Shaping::Advanced).into(),
        );
        let content: Element<'a, Message> = Row::with_children(parts)
            .spacing(config.layout.horizontal_widget_spacing as f32 * 0.6)
            .align_y(Alignment::Center)
            .into();

        let indicator_color = if w.is_urgent {
            Some(config.palette.critical_tint.0)
        } else if w.is_focused {
            Some(config.palette.power_tint.0)
        } else {
            None
        };
        let entry = state_pill(config, content, indicator_color);

        // Clicking a window focuses it (odin's Windowlist `onActivated`).
        mouse_area(entry).on_press(Message::FocusWindow(w.id)).into()
    });
    Some(
        Row::with_children(entries)
            .spacing(config.layout.horizontal_widget_spacing as f32 * 2.0)
            .align_y(Alignment::Center)
            .into(),
    )
}

/// A window's icon, resolved from its `app_id` via the installed `.desktop` entries and
/// the icon theme (odin's `WindowIcon._resolveAppId`). `None` before the desktop-entry
/// scan completes, or when nothing resolves (odin's generic fallback icon still applies
/// inside `icon_for`, so this is only `None` on that final failure — kept as a graceful
/// gap rather than a broken-image placeholder).
fn window_icon<'a>(
    config: &Config,
    desktop_entries: Option<&DesktopEntries>,
    w: &niri::WinEntry,
) -> Option<Element<'a, Message>> {
    // TEMP DIAGNOSTIC: icons disabled to test whether iced_layershell's single-window
    // image-allocation (multi_window.rs always allocates into window_manager.iter_mut()
    // .next(), never the specific requesting surface) is stalling the bottom bar's frame
    // cycle. Remove this early return once confirmed either way.
    return None;
    #[allow(unreachable_code)]
    {
        let path = desktop_entries?.icon_for(&w.app_id)?;
        let side = config.image_icon_size() as f32;
        Some(image(image::Handle::from_path(path)).width(side).height(side).into())
    }
}

/// A short label for a window: its title, else its app id.
fn window_label(w: &niri::WinEntry) -> String {
    let raw = if w.title.is_empty() { &w.app_id } else { &w.title };
    let trimmed: String = raw.chars().take(24).collect();
    if raw.chars().count() > 24 {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
