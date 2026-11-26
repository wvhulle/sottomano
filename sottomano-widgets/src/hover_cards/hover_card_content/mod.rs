mod card_figure_views;
mod pinned_card_header;
mod source_delivery_rows;

use std::iter::once;

use corredo::{
    changing_text::{TextPresence, TransitionStyle, TransitioningText},
    orientation::Axis,
    selection_scroll::{ScrollEdgeFade, WheelAxes},
    theming::{FrameStyle, LINE_HEIGHT, secondary_text_color, success_color},
    width_layout::equal_width_column,
};
use iced::{
    Element, Padding, Pixels,
    alignment::Vertical,
    widget::{Column, Space, Text, container, row, text, text::Wrapping},
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_menus::menu_rows::menu_row::menu_metrics,
    hover_cards::{
        PopupMessage, PressAction,
        hover_card_body::{
            LineOverflow, Section, SectionTitle,
            hover_card_source_delivery::{HoverCard, SectionSource, SourceDelivery},
        },
    },
    icon_catalog::window_icons::WindowAssets,
    widgets::{
        configured_scroll::configured_scroll,
        text::{hover_card_text_effects, panel_text, resolved_font_faces, resolved_text_face},
        value_cells::wrapped_text,
    },
};

const VALUE_COLUMN_WIDTH_PX: f32 = 620.0;

#[derive(Clone, Copy)]
pub struct HoverCardContent<'card> {
    hover_card: &'card HoverCard,
    config: &'card Config,
    assets: WindowAssets<'card>,
    clock: Clock,
    presence: TextPresence,
}

impl<'card> HoverCardContent<'card> {
    #[must_use]
    pub fn new(
        hover_card: &'card HoverCard,
        config: &'card Config,
        assets: WindowAssets<'card>,
        clock: Clock,
    ) -> Self {
        Self {
            hover_card,
            config,
            assets,
            clock,
            presence: TextPresence::Shown,
        }
    }

    fn in_delivery_state(self, delivery: SourceDelivery) -> Self {
        let presence = match delivery {
            SourceDelivery::Delivered(_) | SourceDelivery::Failed => TextPresence::Shown,
            SourceDelivery::Loading => TextPresence::Placeholder,
        };
        Self { presence, ..self }
    }

    fn transition(&self, presence: TextPresence) -> TransitionStyle {
        TransitionStyle {
            effects: hover_card_text_effects(self.config.hover_cards.loading),
            presence,
            highlight: success_color,
        }
    }

    #[must_use]
    pub fn view(self, frame: FrameStyle) -> Element<'static, PopupMessage> {
        let title_rows = self.card_header_rows();
        let mut previous_source = SectionSource::Local;
        let section_views: Vec<Element<'static, PopupMessage>> = self
            .hover_card
            .sections
            .iter()
            .map(|section| {
                let source = section.source;
                let source_row = match source {
                    SectionSource::Snapshot {
                        source_name,
                        delivery,
                    } if !same_source(previous_source, source) => {
                        Some(self.source_row(source_name, delivery))
                    }
                    SectionSource::Snapshot { .. } | SectionSource::Local => None,
                };
                previous_source = source;
                self.section(section, source_row, frame)
            })
            .collect();
        let content = equal_width_column(section_views)
            .spacing(self.config.theme.spacing.group_gap)
            .into();
        configured_scroll(
            self.shell_below_title(title_rows, content),
            Axis::Vertical,
            self.config,
        )
        .edge_fade(ScrollEdgeFade::within_frame(
            frame,
            self.scroll_fade_extent_px(),
        ))
        .into()
    }

    fn scroll_fade_extent_px(&self) -> f32 {
        self.config.group_menus.scroll_fade_rows * menu_metrics(self.config).row_height_px
    }

    fn shell_below_title(
        &self,
        title_rows: Option<Element<'static, PopupMessage>>,
        content: Element<'static, PopupMessage>,
    ) -> Element<'static, PopupMessage> {
        container(
            equal_width_column(title_rows.into_iter().chain(once(content)))
                .spacing(self.config.theme.spacing.item_gap),
        )
        .padding(self.shell_padding())
        .into()
    }

    pub(crate) fn press_actions_table(
        &self,
        actions: &[PressAction],
    ) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        Column::with_children(actions.iter().map(|action| {
            row([
                container(
                    IconGlyph::from(action.gesture.glyph())
                        .view(font.small_text_size_px, secondary_text_color),
                )
                .width(font.icon_size_px)
                .into(),
                self.small_dim_text(action.does.clone().into_owned()).into(),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center)
            .into()
        }))
        .spacing(self.config.theme.spacing.item_gap)
        .into()
    }

    pub(crate) fn loading_placeholder(&self) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let line_height_px = LINE_HEIGHT.to_absolute(Pixels(font.text_size_px)).0;
        TransitioningText::new(
            container(Space::new())
                .width(VALUE_COLUMN_WIDTH_PX)
                .height(line_height_px),
            String::new(),
            self.transition(TextPresence::Placeholder),
            resolved_text_face(font),
            secondary_text_color,
        )
        .into()
    }

    fn content_inset(&self) -> Padding {
        let inset = self.config.theme.spacing.surface_inset;
        Padding::ZERO.horizontal(inset)
    }

    fn shell_padding(&self) -> Padding {
        let inset = self.config.theme.spacing.surface_inset;
        Padding::ZERO.vertical(inset)
    }

    fn title_padding(&self) -> Padding {
        Padding::ZERO.left(self.config.theme.spacing.item_gap)
    }

    fn edge_row_padding(&self) -> Padding {
        Padding::ZERO.horizontal(self.config.theme.spacing.surface_inset)
    }

    fn inset(&self, content: Element<'static, PopupMessage>) -> Element<'static, PopupMessage> {
        container(content).padding(self.content_inset()).into()
    }

    fn small_text(&self, text: String) -> Text<'static> {
        panel_text(&self.config.theme.font, text).size(self.config.theme.font.small_text_size_px)
    }

    fn small_dim_text(&self, text: String) -> Text<'static> {
        self.small_text(text).style(text::secondary)
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        panel_text(&self.config.theme.font, title.to_owned())
            .font(resolved_font_faces(&self.config.theme.font).bold)
            .style(text::base)
    }

    fn section(
        &self,
        section: &Section,
        source_row: Option<Element<'static, PopupMessage>>,
        frame: FrameStyle,
    ) -> Element<'static, PopupMessage> {
        let delivered = match section.source {
            SectionSource::Snapshot { delivery, .. } => self.in_delivery_state(delivery),
            SectionSource::Local => *self,
        };
        let drawn_widget = (section.widget.draw)(&delivered);
        let section_view = match section.line_overflow {
            LineOverflow::Wrap => drawn_widget,
            LineOverflow::ScrollSideways => {
                configured_scroll(drawn_widget, Axis::Horizontal, self.config)
                    .wheel_axes(WheelAxes::AlongOnly)
                    .edge_fade(ScrollEdgeFade::away_from_frame(
                        frame,
                        self.scroll_fade_extent_px(),
                    ))
                    .into()
            }
        };
        self.inset(
            Column::with_children(
                self.title_rows(&section.title)
                    .into_iter()
                    .chain(source_row)
                    .chain(once(section_view)),
            )
            .spacing(self.config.theme.spacing.item_gap)
            .into(),
        )
    }

    fn title_rows(&self, title: &SectionTitle) -> Vec<Element<'static, PopupMessage>> {
        let title_row = container(self.bold_title(title.text()))
            .padding(self.title_padding())
            .into();
        match title {
            SectionTitle::Bare(_) => vec![title_row],
            SectionTitle::Captioned { caption, .. } => vec![
                title_row,
                container(self.small_dim_text(caption.clone()))
                    .max_width(VALUE_COLUMN_WIDTH_PX)
                    .padding(self.title_padding())
                    .into(),
            ],
        }
    }

    pub(crate) fn paragraph(&self, text: String) -> Element<'static, PopupMessage> {
        wrapped_text(
            &self.config.theme.font,
            text,
            secondary_text_color,
            VALUE_COLUMN_WIDTH_PX,
            Wrapping::default(),
        )
        .into()
    }
}

fn same_source(previous_source: SectionSource, source: SectionSource) -> bool {
    match (previous_source, source) {
        (
            SectionSource::Snapshot {
                source_name: previous_name,
                ..
            },
            SectionSource::Snapshot { source_name, .. },
        ) => previous_name == source_name,
        _ => false,
    }
}
