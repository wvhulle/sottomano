use iced::{
    Border, Color, Element, Padding,
    alignment::Vertical,
    border::Radius,
    widget::{Column, Row as RowWidget, container, text::Wrapping},
};

use crate::{
    app::Message,
    config::Config,
    data::source::{PreviewTile, WorkspacePreview},
    widgets::{
        native_icon::{IconSource, native_icon},
        text::bar_text,
    },
};

const TILE_GAP_PX: f32 = 3.0;
const TILE_LABEL_GAP_PX: f32 = 5.0;
const TILE_LABEL_WIDTH_RATIO: f32 = 2.5;
const TILE_LABEL_SCALE: f32 = 0.8;
const TILE_BORDER_PX: f32 = 1.0;
const TILE_RADIUS_PX: f32 = 4.0;
const TILE_FILL_ALPHA: f32 = 0.18;
const TILE_PADDING: Padding = Padding::new(4.0);

pub fn schematic<'a>(
    preview: &WorkspacePreview,
    config: &Config,
    foreground: Color,
    dim: Color,
    alpha: f32,
) -> Element<'a, Message> {
    let scale = config.widgets.workspace_preview_scale;
    let mut rows = vec![
        RowWidget::with_children(
            preview
                .columns
                .iter()
                .map(|column| stacked_column(column, config, scale, foreground, dim, alpha)),
        )
        .spacing(TILE_GAP_PX)
        .into(),
    ];
    if !preview.floating.is_empty() {
        rows.push(
            RowWidget::with_children(
                preview
                    .floating
                    .iter()
                    .map(|tile| window_tile(tile, config, scale, foreground, dim, alpha)),
            )
            .spacing(TILE_GAP_PX)
            .into(),
        );
    }
    Column::with_children(rows).spacing(TILE_GAP_PX).into()
}

fn stacked_column<'a>(
    tiles: &[PreviewTile],
    config: &Config,
    scale: f32,
    foreground: Color,
    dim: Color,
    alpha: f32,
) -> Element<'a, Message> {
    Column::with_children(
        tiles
            .iter()
            .map(|tile| window_tile(tile, config, scale, foreground, dim, alpha)),
    )
    .spacing(TILE_GAP_PX)
    .into()
}

fn window_tile<'a>(
    tile: &PreviewTile,
    config: &Config,
    scale: f32,
    foreground: Color,
    dim: Color,
    alpha: f32,
) -> Element<'a, Message> {
    let side = config.font.icon_size_px;
    let width = (tile.width_px as f32 * scale).max(side);
    let height = (tile.height_px as f32 * scale).max(side);
    let icon = tile
        .icon
        .as_ref()
        .map(|path| native_icon(&IconSource::Themed(path.clone()), side, alpha));
    let label = (width >= side * TILE_LABEL_WIDTH_RATIO).then(|| {
        bar_text(&config.font, tile.title.clone())
            .size(config.font.text_size_px * TILE_LABEL_SCALE)
            .color(foreground)
            .wrapping(Wrapping::WordOrGlyph)
    });
    let content = RowWidget::with_children(
        icon.into_iter()
            .chain(label.map(Element::from))
            .collect::<Vec<_>>(),
    )
    .spacing(TILE_LABEL_GAP_PX)
    .align_y(Vertical::Top);

    let outline = if tile.is_urgent {
        config.palette.urgent
    } else if tile.is_focused {
        config.palette.focus
    } else {
        dim
    }
    .scale_alpha(alpha);
    let fill = Color {
        a: TILE_FILL_ALPHA,
        ..config.palette.dim
    }
    .scale_alpha(alpha);

    container(content)
        .width(width)
        .height(height)
        .padding(TILE_PADDING)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(fill.into()),
            border: Border {
                color: outline,
                width: TILE_BORDER_PX,
                radius: Radius::new(TILE_RADIUS_PX),
            },
            ..container::Style::default()
        })
        .into()
}
