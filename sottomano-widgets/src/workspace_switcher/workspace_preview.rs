use corredo::theming::{
    background_color, background_strong_color, background_weak_color, danger_color, primary_color,
    secondary_text_color, text_color,
};
use iced::{
    Border, Color, Element, Padding, Shadow, Theme, Vector,
    alignment::Vertical,
    border::Radius,
    widget::{Column, Row as RowWidget, Stack, button, button::Status, container, text::Wrapping},
};
use sottomano_compositor_model::compositor_window_facts::{Urgency, WindowFocus, WindowTiling};
use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::{
    WindowCaption, WindowLabel,
};
use sottomano_toml_config::Config;

use crate::{
    hover_cards::{
        PopupMessage,
        hover_card_body::hover_card_figure_payloads::{
            PlacedPreviewTile, PreviewTile, TileIcon, WorkspaceArrangement, WorkspacePreview,
        },
    },
    icon_catalog::{
        favicons::Favicons,
        window_icons::{WindowAssets, window_icon},
    },
    widgets::{shared_metrics::OUTLINE_WIDTH_PX, window_title::WindowTitle},
};

const TILE_LABEL_WIDTH_RATIO: f32 = 2.5;
const FLOATING_SHADOW_ALPHA: f32 = 0.45;
const FLOATING_SHADOW_OFFSET: Vector = Vector::new(0.0, 2.0);
const FLOATING_SHADOW_BLUR_PX: f32 = 6.0;

pub struct WorkspaceSchematic<'view> {
    preview: &'view WorkspacePreview,
    config: &'view Config,
    favicons: &'view Favicons,
}

impl<'view> WorkspaceSchematic<'view> {
    #[must_use]
    pub fn new(
        preview: &'view WorkspacePreview,
        config: &'view Config,
        assets: WindowAssets<'view>,
    ) -> Self {
        Self {
            preview,
            config,
            favicons: assets.favicons,
        }
    }

    #[must_use]
    pub fn view(self) -> Element<'static, PopupMessage> {
        match &self.preview.arrangement {
            WorkspaceArrangement::ScrollingColumns { columns, floating } => {
                self.scrolling_columns_view(columns, floating)
            }
            WorkspaceArrangement::ScreenGeometry { tiles } => self.screen_geometry_view(tiles),
        }
    }

    fn scrolling_columns_view(
        &self,
        columns: &[Vec<PreviewTile>],
        floating: &[PreviewTile],
    ) -> Element<'static, PopupMessage> {
        let mut rows = vec![
            RowWidget::with_children(columns.iter().map(|column| self.column(column)))
                .spacing(self.tile_gap())
                .into(),
        ];
        if !floating.is_empty() {
            rows.push(
                RowWidget::with_children(floating.iter().map(|tile| self.tile(tile)))
                    .spacing(self.tile_gap())
                    .into(),
            );
        }
        Column::with_children(rows).spacing(self.tile_gap()).into()
    }

    fn screen_geometry_view(&self, tiles: &[PlacedPreviewTile]) -> Element<'static, PopupMessage> {
        let scale = self.config.workspace_switcher.preview_scale;
        Stack::with_children(tiles.iter().map(|placed| {
            container(self.tile(&placed.tile))
                .padding(Padding {
                    top: placed.offset.y * scale,
                    left: placed.offset.x * scale,
                    right: 0.0,
                    bottom: 0.0,
                })
                .into()
        }))
        .into()
    }

    fn tile_gap(&self) -> f32 {
        self.preview.app_window_layout.gaps_px * self.config.workspace_switcher.preview_scale
    }

    fn column(&self, tiles: &[PreviewTile]) -> Element<'static, PopupMessage> {
        Column::with_children(tiles.iter().map(|tile| self.tile(tile)))
            .spacing(self.tile_gap())
            .into()
    }

    fn tile(&self, tile: &PreviewTile) -> Element<'static, PopupMessage> {
        let scale = self.config.workspace_switcher.preview_scale;
        let side = self.config.theme.font.icon_size_px;
        let width = (tile.tile_size.width * scale).max(side);
        let height = (tile.tile_size.height * scale).max(side);

        let resolved_icon = match &tile.icon {
            TileIcon::Resolved(icon) => Some(icon),
            TileIcon::Unresolved => None,
        };
        let icon = window_icon(
            self.favicons,
            tile.caption.favicon_key(),
            resolved_icon,
            side,
            1.0,
        );
        let label = match &tile.caption {
            WindowCaption::Labelled(label) if width >= side * TILE_LABEL_WIDTH_RATIO => {
                Some(self.label(label))
            }
            WindowCaption::Labelled(_) | WindowCaption::Unlabelled => None,
        };
        let content = RowWidget::with_children(icon.into_iter().chain(label).collect::<Vec<_>>())
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Top);

        let outline = tile_outline(tile);
        let radius = Radius::from(self.preview.app_window_layout.window_corner_radius_px);
        let tile_layer = match tile.tiling {
            WindowTiling::Floating => TileLayer::Floating,
            WindowTiling::Tiled => TileLayer::Tiled,
        };

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
                    shadow: floating_lift(theme, tile_layer),
                    ..button::Style::default()
                }
            })
            .into()
    }

    fn label(&self, label: &WindowLabel) -> Element<'static, PopupMessage> {
        WindowTitle::new(label, self.config)
            .size(self.config.theme.font.small_text_size_px)
            .wrapping(Wrapping::WordOrGlyph)
            .group_key_above_title()
            .into()
    }
}

fn tile_outline(tile: &PreviewTile) -> fn(&Theme) -> Color {
    match (tile.urgency, tile.focus) {
        (Urgency::Urgent, _) => danger_color,
        (Urgency::NotUrgent, WindowFocus::Focused) => primary_color,
        (Urgency::NotUrgent, WindowFocus::Unfocused) => secondary_text_color,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TileLayer {
    Floating,
    Tiled,
}

fn floating_lift(theme: &Theme, tile_layer: TileLayer) -> Shadow {
    if tile_layer == TileLayer::Tiled {
        return Shadow::default();
    }
    Shadow {
        color: background_color(theme).scale_alpha(FLOATING_SHADOW_ALPHA),
        offset: FLOATING_SHADOW_OFFSET,
        blur_radius: FLOATING_SHADOW_BLUR_PX,
    }
}
