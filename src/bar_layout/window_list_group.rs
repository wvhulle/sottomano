//! Niri window list.

use std::{collections::HashMap, path::PathBuf};

use iced::{
    Alignment, Element,
    font::Style as FontStyle,
    widget::{Row, image, mouse_area, rich_text, svg},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::{
        app_rules::{self, AppRule, IconCandidate, TitleParts},
        niri,
    },
    layer_surface::Position,
    user_config::{Config, font},
    widgets::{activity_line::activity_line, pager_slot::PagerSlot, text::fallback_spans},
};

pub fn view<'a>(
    niri: &EventStreamState,
    output: &str,
    config: &Config,
    app_icons: &HashMap<String, PathBuf>,
    theme_icons: &HashMap<String, PathBuf>,
    favicons: &HashMap<String, PathBuf>,
    position: Position,
) -> Option<Element<'a, Message>> {
    let workspace = niri::active_workspace_id(niri, output)?;
    let windows = niri::windows_on_workspace(niri, workspace);
    if windows.is_empty() {
        return None;
    }
    let entries = windows.into_iter().map(|w| {
        let rule = app_rules::rule_for(&w.app_id);
        let label = if w.title.is_empty() {
            w.app_id.clone()
        } else {
            w.title.clone()
        };
        let parts = rule.rewrite(&label);
        let favicon = rule
            .favicon_key(&w.title)
            .and_then(|key| favicons.get(&key));

        let mut row: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = window_icon(
            config,
            app_icons,
            theme_icons,
            rule,
            &w,
            favicon.map(AsRef::as_ref),
        ) {
            row.push(icon);
        }
        row.push(title_element(config, &parts));
        let content: Element<'a, Message> = Row::with_children(row)
            .spacing(config.layout.horizontal_widget_spacing_px * 0.6)
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
            config.layout.focus_underline_thickness_px,
        );

        mouse_area(PagerSlot::new(content, focus).view(position))
            .on_press(Message::FocusWindow(w.id))
            .into()
    });
    Some(
        Row::with_children(entries)
            .spacing(config.layout.horizontal_widget_spacing_px * 2.0)
            .align_y(Alignment::Center)
            .into(),
    )
}

/// Lead (muted, italic) + "·" + main, mirroring odin's `WindowTitleContext` /
/// `WindowTitleBody`. The lead block is skipped entirely when there is no
/// rewritten prefix, so the main text sits alone with no stray dot.
fn title_element<'a>(config: &Config, parts: &TitleParts) -> Element<'a, Message> {
    let dim = config.palette.dim();
    let lead = (!parts.lead.is_empty()).then(|| {
        let italic = iced::Font {
            style: FontStyle::Italic,
            ..font::iced_font()
        };
        fallback_spans(&parts.lead, italic, Some(dim))
            .into_iter()
            .chain(fallback_spans(" · ", font::iced_font(), Some(dim)))
    });
    let spans: Vec<_> = lead
        .into_iter()
        .flatten()
        .chain(fallback_spans(&parts.main, font::iced_font(), None))
        .collect();
    rich_text(spans).size(font::size()).into()
}

fn window_icon<'a>(
    config: &Config,
    app_icons: &HashMap<String, PathBuf>,
    theme_icons: &HashMap<String, PathBuf>,
    rule: AppRule,
    w: &niri::WinEntry,
    favicon: Option<&std::path::Path>,
) -> Option<Element<'a, Message>> {
    let side = config.font.icon_size_px;
    let path = rule
        .icon_candidates(w, favicon)
        .into_iter()
        .find_map(|c| resolve_candidate(app_icons, theme_icons, c))?;
    // Icon themes (e.g. Colloid) ship scalable SVGs; hicolor app icons and
    // browser favicons are raster. iced's `image` widget only decodes raster
    // formats, so SVGs must go through the `svg` widget or they render blank.
    let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    Some(if is_svg {
        svg(svg::Handle::from_path(path))
            .width(side)
            .height(side)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .into()
    })
}

fn resolve_candidate(
    app_icons: &HashMap<String, PathBuf>,
    theme_icons: &HashMap<String, PathBuf>,
    candidate: IconCandidate,
) -> Option<PathBuf> {
    match candidate {
        IconCandidate::Path(path) => Some(path),
        IconCandidate::Theme(name) => theme_icons.get(&name).cloned(),
        IconCandidate::AppId(app_id) => app_icons.get(&app_id).cloned(),
    }
}
