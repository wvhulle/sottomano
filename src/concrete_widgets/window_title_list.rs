//! The niri window list (odin's `Windowlist` ConcreteWidget).

use iced::widget::text::Shaping;
use iced::widget::{Row, image, mouse_area, text};
use iced::{Alignment, Color, Element};

use crate::bar_program::Message;
use crate::data_sources::niri;
use crate::visual_elements::{edge_overlay, font_family, hairline};

use super::Context;

/// The window list (odin's `Windowlist`): icon + title of the windows on the output's
/// active workspace, in on-screen order. The focused entry carries a hairline indicator
/// along the inner edge (powerTint), an urgent one criticalTint — the same sliding-focus
/// affordance the pager uses, not a text-color highlight (odin's underline, not tint,
/// marks focus; the label itself only dims for inactive entries). `None` when the
/// workspace has no windows. Clicking an entry focuses that window.
pub fn window_list<'a>(ctx: &Context<'a>) -> Option<Element<'a, Message>> {
    let workspace = niri::active_workspace_id(ctx.niri, ctx.output)?;
    let windows = niri::windows_on_workspace(ctx.niri, workspace);
    if windows.is_empty() {
        return None;
    }
    let size = ctx.config.font.size as f32;
    let sans = font_family(&ctx.config.font.family);
    let entries = windows.into_iter().map(|w| {
        let label_color = if w.is_focused || w.is_urgent {
            Color::WHITE
        } else {
            ctx.config.palette.muted.0
        };
        let mut parts: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = window_icon(ctx, &w) {
            parts.push(icon);
        }
        parts.push(
            text(window_label(&w)).size(size).font(sans).color(label_color).shaping(Shaping::Advanced).into(),
        );
        let content: Element<'a, Message> = Row::with_children(parts)
            .spacing(ctx.config.layout.horizontal_widget_spacing as f32 * 0.6)
            .align_y(Alignment::Center)
            .into();

        // Always the same widget tree shape (a Stack with a hairline layer) — see the
        // comment on the same pattern in `workspace_pager::workspaces` for why the
        // indicator must never be structurally omitted.
        let (color, opacity) = if w.is_urgent {
            (ctx.config.palette.critical_tint.0, 1.0)
        } else if w.is_focused {
            (ctx.config.palette.power_tint.0, 1.0)
        } else {
            (ctx.config.palette.power_tint.0, 0.0)
        };
        let entry = edge_overlay(ctx, content, hairline(ctx, color, opacity));

        // Clicking a window focuses it (odin's Windowlist `onActivated`).
        mouse_area(entry).on_press(Message::FocusWindow(w.id)).into()
    });
    Some(
        Row::with_children(entries)
            .spacing(ctx.config.layout.horizontal_widget_spacing as f32 * 2.0)
            .align_y(Alignment::Center)
            .into(),
    )
}

/// A window's icon, resolved from its `app_id` via the installed `.desktop` entries and
/// the icon theme (odin's `WindowIcon._resolveAppId`). `None` before the desktop-entry
/// scan completes, or when nothing resolves (odin's generic fallback icon still applies
/// inside `icon_for`, so this is only `None` on that final failure — kept as a graceful
/// gap rather than a broken-image placeholder).
fn window_icon<'a>(ctx: &Context<'a>, w: &niri::WinEntry) -> Option<Element<'a, Message>> {
    let path = ctx.desktop_entries?.icon_for(&w.app_id)?;
    let side = ctx.config.image_icon_size() as f32;
    Some(image(image::Handle::from_path(path)).width(side).height(side).into())
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
