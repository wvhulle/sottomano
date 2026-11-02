use iced::{
    Border, Color, Element, Shadow, Theme, Vector,
    alignment::Vertical,
    border::Radius,
    widget::{Column, Row as RowWidget, button, button::Status, text::Wrapping},
};
use sottomano_config::Config;
use sottomano_sources::desktop_windows_apps::window_title_labels::WindowLabel;

use crate::{
    hover_cards::{
        PopupMessage,
        hover_card_body::hover_card_figure_payloads::{PreviewTile, WorkspacePreview},
    },
    widgets::{
        configured_theme::{
            background_color, background_strong_color, background_weak_color, danger_color,
            primary_color, secondary_text_color, text_color,
        },
        favicon::Favicons,
        shared_metrics::OUTLINE_WIDTH_PX,
        window_icon::{WindowAssets, window_icon},
        window_title::WindowTitle,
    },
};

const TILE_LABEL_WIDTH_RATIO: f32 = 2.5;
const FLOATING_SHADOW_ALPHA: f32 = 0.45;
const FLOATING_SHADOW_OFFSET: Vector = Vector::new(0.0, 2.0);
const FLOATING_SHADOW_BLUR_PX: f32 = 6.0;

pub struct WorkspaceSchematic<'a> {
    preview: &'a WorkspacePreview,
    config: &'a Config,
    favicons: &'a Favicons,
}

impl<'a> WorkspaceSchematic<'a> {
    #[must_use]
    pub fn new(
        preview: &'a WorkspacePreview,
        config: &'a Config,
        assets: WindowAssets<'a>,
    ) -> Self {
        Self {
            preview,
            config,
            favicons: assets.favicons,
        }
    }

    #[must_use]
    pub fn view(self) -> Element<'static, PopupMessage> {
        let mut rows = vec![
            RowWidget::with_children(
                self.preview
                    .columns
                    .iter()
                    .map(|column| self.column(column)),
            )
            .spacing(self.tile_gap())
            .into(),
        ];
        if !self.preview.floating.is_empty() {
            rows.push(
                RowWidget::with_children(self.preview.floating.iter().map(|tile| self.tile(tile)))
                    .spacing(self.tile_gap())
                    .into(),
            );
        }
        Column::with_children(rows).spacing(self.tile_gap()).into()
    }

    fn tile_gap(&self) -> f32 {
        self.preview.niri_layout.gaps_px * self.config.widget_groups.workspaces.preview_scale
    }

    fn column(&self, tiles: &[PreviewTile]) -> Element<'static, PopupMessage> {
        Column::with_children(tiles.iter().map(|tile| self.tile(tile)))
            .spacing(self.tile_gap())
            .into()
    }

    fn tile(&self, tile: &PreviewTile) -> Element<'static, PopupMessage> {
        let scale = self.config.widget_groups.workspaces.preview_scale;
        let side = self.config.theme.font.icon_size_px;
        let width = (tile.tile_size.width * scale).max(side);
        let height = (tile.tile_size.height * scale).max(side);

        let icon = window_icon(
            self.favicons,
            tile.favicon_key.as_deref(),
            tile.icon.as_deref(),
            side,
            1.0,
        );
        let label = tile
            .label
            .as_ref()
            .filter(|_| width >= side * TILE_LABEL_WIDTH_RATIO)
            .map(|label| self.label(label));
        let content = RowWidget::with_children(icon.into_iter().chain(label).collect::<Vec<_>>())
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Top);

        let outline = tile_outline(tile);
        let radius = Radius::from(self.preview.niri_layout.window_corner_radius_px);
        let floating = tile.is_floating;

        button(content)
            .width(width)
            .height(height)
            .padding(self.config.theme.spacing.item_gap)
            .clip(true)
            .on_press(PopupMessage::FocusWindow(tile.window))
            .style(move |theme, status| {
                let hovered = matches!(status, Status::Hovered | Status::Pressed);
                let fill = if hovered {
                    background_strong_color(theme)
                } else {
                    background_weak_color(theme)
                };
                button::Style {
                    background: Some(fill.into()),
                    text_color: text_color(theme),
                    border: Border {
                        color: if hovered {
                            primary_color(theme)
                        } else {
                            outline(theme)
                        },
                        width: OUTLINE_WIDTH_PX,
                        radius,
                    },
                    shadow: floating_lift(theme, floating),
                    ..button::Style::default()
                }
            })
            .into()
    }

    fn label(&self, label: &WindowLabel) -> Element<'static, PopupMessage> {
        WindowTitle::new(label, self.config)
            .size(self.config.theme.font.small_text_size_px)
            .wrapping(Wrapping::WordOrGlyph)
            .location_above_title()
            .into()
    }
}

fn tile_outline(tile: &PreviewTile) -> fn(&Theme) -> Color {
    if tile.is_urgent {
        danger_color
    } else if tile.is_focused {
        primary_color
    } else {
        secondary_text_color
    }
}

fn floating_lift(theme: &Theme, floating: bool) -> Shadow {
    if !floating {
        return Shadow::default();
    }
    Shadow {
        color: background_color(theme).scale_alpha(FLOATING_SHADOW_ALPHA),
        offset: FLOATING_SHADOW_OFFSET,
        blur_radius: FLOATING_SHADOW_BLUR_PX,
    }
}
