//! The niri workspace pager (odin's `Workspaces` ConcreteWidget/WidgetGroup). Stateless —
//! always renders live niri state, so unlike battery/system it has no owned fields.

use iced::widget::text::Shaping;
use iced::widget::{Row, button, row, text};
use iced::{Alignment, Color, Element};
use niri_ipc::state::EventStreamState;

use crate::bar_program::Message;
use crate::data_sources::niri;
use crate::user_config::Config;
use crate::visual_elements::{button_style, font_family};

/// The workspace pager (odin's `Workspaces`): this output's workspaces in index order, as
/// clickable slots. Focus and urgency are shown by the slot button's own background
/// (powerTint / criticalTint), not by tinting the label — so labels stay neutral,
/// matching odin. Clicking a workspace focuses it.
pub fn view<'a>(niri: &EventStreamState, output: &str, config: &Config) -> Element<'a, Message> {
    let slots = niri::workspaces_for_output(niri, output).into_iter().map(|p| {
        let label = workspace_label(config, &p);
        let color = if p.is_urgent {
            Some(config.palette.critical_tint.0)
        } else if p.is_active {
            Some(config.palette.power_tint.0)
        } else {
            None
        };
        button(label)
            .padding([2.0, config.layout.horizontal_widget_spacing as f32 * 0.5])
            .on_press(Message::FocusWorkspace(p.id))
            .style(move |_theme, status| button_style(color, status))
            .into()
    });
    Row::with_children(slots)
        .spacing(config.layout.horizontal_widget_spacing as f32 * 1.5)
        .align_y(Alignment::Center)
        .into()
}

/// A workspace's neutral label (odin's slot content): a named workspace in the
/// keyboard-reachable range (index ≤ 10, the 1–9/0 keys) gets a small raised index prefix
/// before its name; otherwise just the name, or the bare index when unnamed.
fn workspace_label<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, Message> {
    let size = config.font.size as f32;
    let sans = font_family(&config.font.family);
    // Workspace 10 is bound to "0"; above 10 the number is useless as a hint.
    let key = if p.idx == 10 { "0".to_string() } else { p.idx.to_string() };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    match &p.name {
        Some(name) if named && p.idx <= 10 => {
            // Smaller, dimmed, top-aligned index reads as a superscript before the name.
            let dim = Color { a: 0.5, ..Color::WHITE };
            row![
                text(key).size(size * 0.7).font(sans).color(dim).shaping(Shaping::Advanced),
                text(name.clone()).size(size).font(sans).shaping(Shaping::Advanced),
            ]
            .align_y(Alignment::Start)
            .into()
        }
        Some(name) if named => {
            text(name.clone()).size(size).font(sans).shaping(Shaping::Advanced).into()
        }
        _ => text(key).size(size).font(sans).shaping(Shaping::Advanced).into(),
    }
}
