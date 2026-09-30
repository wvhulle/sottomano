use iced::{
    Border, Color, Element, Padding, Shadow, Vector,
    alignment::Vertical,
    border::Radius,
    widget::{Column, Row as RowWidget, button, button::Status, rich_text, text::Wrapping},
};

use crate::{
    app::Message,
    config::Config,
    widgets::{
        favicon::Favicons,
        text::LINE_HEIGHT,
        tooltip_body::{PreviewTile, WorkspacePreview},
        window_icon::window_icon,
        window_title::title_spans,
    },
};

const TILE_GAP_PX: f32 = 3.0;
const TILE_LABEL_GAP_PX: f32 = 5.0;
const TILE_LABEL_WIDTH_RATIO: f32 = 2.5;
const TILE_LABEL_SCALE: f32 = 0.8;
const TILE_BORDER_PX: f32 = 2.0;
const TILE_FILL_ALPHA: f32 = 0.18;
const TILE_HOVER_FILL_ALPHA: f32 = 0.34;
const TILE_OUTLINE_ALPHA: f32 = 0.7;
const TILE_PADDING: Padding = Padding::new(4.0);
const FLOATING_SHADOW_ALPHA: f32 = 0.45;
const FLOATING_SHADOW_OFFSET: Vector = Vector::new(0.0, 2.0);
const FLOATING_SHADOW_BLUR_PX: f32 = 6.0;

pub struct WorkspaceSchematic<'a> {
    preview: &'a WorkspacePreview,
    config: &'a Config,
    corner_radius: Radius,
    favicons: &'a Favicons,
}

impl<'a> WorkspaceSchematic<'a> {
    pub fn new(
        preview: &'a WorkspacePreview,
        config: &'a Config,
        corner_radius: Radius,
        favicons: &'a Favicons,
    ) -> Self {
        Self {
            preview,
            config,
            corner_radius,
            favicons,
        }
    }

    pub fn view(self) -> Element<'static, Message> {
        let mut rows = vec![
            RowWidget::with_children(
                self.preview
                    .columns
                    .iter()
                    .map(|column| self.column(column)),
            )
            .spacing(TILE_GAP_PX)
            .into(),
        ];
        if !self.preview.floating.is_empty() {
            rows.push(
                RowWidget::with_children(self.preview.floating.iter().map(|tile| self.tile(tile)))
                    .spacing(TILE_GAP_PX)
                    .into(),
            );
        }
        Column::with_children(rows).spacing(TILE_GAP_PX).into()
    }

    fn column(&self, tiles: &[PreviewTile]) -> Element<'static, Message> {
        Column::with_children(tiles.iter().map(|tile| self.tile(tile)))
            .spacing(TILE_GAP_PX)
            .into()
    }

    fn tile(&self, tile: &PreviewTile) -> Element<'static, Message> {
        let scale = self.config.widgets.workspace_preview_scale;
        let side = self.config.font.icon_size_px;
        let width = (tile.tile_size.width * scale).max(side);
        let height = (tile.tile_size.height * scale).max(side);

        let icon = window_icon(
            self.favicons,
            tile.favicon_key.as_deref(),
            tile.icon.as_deref(),
            side,
            1.0,
        );
        let label = (width >= side * TILE_LABEL_WIDTH_RATIO).then(|| self.label(tile));
        let content = RowWidget::with_children(icon.into_iter().chain(label).collect::<Vec<_>>())
            .spacing(TILE_LABEL_GAP_PX)
            .align_y(Vertical::Top);

        let outline = self.outline(tile);
        let highlight = self.config.palette.focus;
        let fill = Color {
            a: TILE_FILL_ALPHA,
            ..self.config.palette.dim
        };
        let hovered_fill = Color {
            a: TILE_HOVER_FILL_ALPHA,
            ..self.config.palette.dim
        };
        let radius = self.corner_radius;
        let shadow = self.lift(tile);
        let text_color = self.config.palette.bar_foreground;

        button(content)
            .width(width)
            .height(height)
            .padding(TILE_PADDING)
            .clip(true)
            .on_press(Message::FocusWindow(tile.window))
            .style(move |_, status| {
                let hovered = matches!(status, Status::Hovered | Status::Pressed);
                button::Style {
                    background: Some(if hovered { hovered_fill } else { fill }.into()),
                    text_color,
                    border: Border {
                        color: if hovered { highlight } else { outline },
                        width: TILE_BORDER_PX,
                        radius,
                    },
                    shadow,
                    ..button::Style::default()
                }
            })
            .into()
    }

    fn lift(&self, tile: &PreviewTile) -> Shadow {
        if !tile.is_floating {
            return Shadow::default();
        }
        Shadow {
            color: Color {
                a: FLOATING_SHADOW_ALPHA,
                ..self.config.palette.surface
            },
            offset: FLOATING_SHADOW_OFFSET,
            blur_radius: FLOATING_SHADOW_BLUR_PX,
        }
    }

    fn label(&self, tile: &PreviewTile) -> Element<'static, Message> {
        rich_text(title_spans(self.config, &tile.title, 1.0))
            .size(self.config.font.text_size_px * TILE_LABEL_SCALE)
            .line_height(LINE_HEIGHT)
            .wrapping(Wrapping::WordOrGlyph)
            .into()
    }

    fn outline(&self, tile: &PreviewTile) -> Color {
        if tile.is_urgent {
            self.config.palette.urgent
        } else if tile.is_focused {
            self.config.palette.focus
        } else {
            Color {
                a: TILE_OUTLINE_ALPHA,
                ..self.config.palette.bar_foreground
            }
        }
    }
}
