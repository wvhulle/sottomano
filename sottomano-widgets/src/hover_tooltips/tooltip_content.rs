use std::{
    iter::{empty, once},
    time::{Duration, SystemTime},
};

use iced::{
    Element, Length, Padding, Pixels,
    alignment::Vertical,
    font::Style as FontStyle,
    mouse::Interaction,
    widget::{Column, Space, Text, container, row, text, text::Wrapping},
};
use iced_controller_diagram::{Style as DiagramStyle, controller_diagram};
use sottomano_config::Config;

use crate::{
    clock::Clock,
    hover_tooltips::{MouseGesture, PopupMessage, PressHints, core_grid::CoreCellGrid},
    widget_groups::{
        window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        bar_widget::Glyph,
        configured_theme::{
            primary_color, primary_shades, secondary_text_color, success_color, success_shades,
            text_color,
        },
        keyed_record_table::{KeyedRecordTable, KeyedRecordTableStyle},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        pressable_text::pressable_text,
        refresh_dial::RefreshDial,
        text::{FontFaces, LINE_HEIGHT, bar_text},
        tooltip_body::{
            Heading, Section, SectionBody, SectionHeading,
            tooltip_figure_payloads::{ChipletCells, ControllerCallouts},
            tooltip_source_delivery::{
                DeliveryTiming, SectionsDeliveryState, SourcedSections, Tooltip,
            },
        },
        transitioning_text::{TextPresence, TransitionStyle, TransitioningText},
        uniform_width_column::uniform_width_column,
        value_cells::{ValueCellPressMessages, ValueCellView, wrapped_text},
        widget_state::WidgetState,
        window_icon::WindowAssets,
    },
};

const COLUMN_GAP_PX: f32 = 14.0;
const SECTION_SEPARATION_PX: f32 = 16.0;
const CONTROLLER_DIAGRAM_WIDTH_PX: f32 = 300.0;
const GESTURE_ICON_GAP_PX: f32 = 4.0;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 0.0,
    bottom: 2.0,
    left: COLUMN_GAP_PX / 2.0,
};

#[derive(Clone, Copy)]
pub struct TooltipContent<'a> {
    tooltip: &'a Tooltip,
    config: &'a Config,
    assets: WindowAssets<'a>,
    clock: Clock,
    presence: TextPresence,
}

impl<'a> TooltipContent<'a> {
    #[must_use]
    pub fn new(
        tooltip: &'a Tooltip,
        config: &'a Config,
        assets: WindowAssets<'a>,
        clock: Clock,
    ) -> Self {
        Self {
            tooltip,
            config,
            assets,
            clock,
            presence: TextPresence::Shown,
        }
    }

    fn in_delivery_state(self, delivery_state: SectionsDeliveryState) -> Self {
        let presence = match delivery_state {
            SectionsDeliveryState::Delivered { .. } => TextPresence::Shown,
            SectionsDeliveryState::AwaitingDelivery => TextPresence::Placeholder,
        };
        Self { presence, ..self }
    }

    fn transition(&self, presence: TextPresence) -> TransitionStyle {
        TransitionStyle {
            loading: self.config.tooltips.loading,
            presence,
            highlight: success_color,
        }
    }

    #[must_use]
    pub fn view(self) -> Element<'static, PopupMessage> {
        self.arranged_around(empty())
    }

    #[must_use]
    pub fn view_enclosing<'panel, Message: From<PopupMessage> + 'panel>(
        self,
        panel: Element<'panel, Message>,
    ) -> Element<'panel, Message> {
        self.arranged_around(once(panel))
    }

    fn arranged_around<'panel, Message: From<PopupMessage> + 'panel>(
        self,
        enclosed_panel: impl IntoIterator<Item = Element<'panel, Message>>,
    ) -> Element<'panel, Message> {
        let lift = |element: Element<'static, PopupMessage>| -> Element<'panel, Message> {
            let element: Element<'panel, PopupMessage> = element;
            element.map(Message::from)
        };
        let groups: Vec<&SourcedSections> = self.tooltip.filled_groups().collect();
        let Some((closing_group, leading_groups)) = groups.split_last() else {
            return container(uniform_width_column(
                enclosed_panel
                    .into_iter()
                    .chain(once(lift(self.closing_row(None)))),
            ))
            .padding(self.shell_padding())
            .into();
        };
        let leading = leading_groups.iter().map(|group| {
            lift(
                uniform_width_column([
                    self.group_sections(group),
                    self.footer_row(None, Some(self.delivery_state_view(group.delivery_state()))),
                ])
                .spacing(self.config.layout.overlay.row_spacing)
                .into(),
            )
        });
        let closing: Element<'panel, Message> = uniform_width_column(
            once(lift(self.group_sections(closing_group)))
                .chain(enclosed_panel)
                .chain(once(lift(
                    self.closing_row(Some(closing_group.delivery_state())),
                ))),
        )
        .spacing(SECTION_SEPARATION_PX / 2.0)
        .into();
        container(uniform_width_column(leading.chain(once(closing))).spacing(SECTION_SEPARATION_PX))
            .padding(self.shell_padding())
            .into()
    }

    fn group_sections(&self, group: &SourcedSections) -> Element<'static, PopupMessage> {
        let content = self.in_delivery_state(group.delivery_state());
        let source_heading = group.source_heading().map(|heading| {
            self.inset(
                Column::with_children(self.heading_rows(heading))
                    .spacing(self.config.layout.overlay.row_spacing)
                    .into(),
            )
        });
        let sections: Element<'static, PopupMessage> = uniform_width_column(
            group
                .filled_sections()
                .map(|section| content.section(section)),
        )
        .spacing(SECTION_SEPARATION_PX)
        .into();
        uniform_width_column(source_heading.into_iter().chain(once(sections)))
            .spacing(self.config.layout.overlay.row_spacing)
            .into()
    }

    fn closing_row(
        &self,
        closing_delivery_state: Option<SectionsDeliveryState>,
    ) -> Element<'static, PopupMessage> {
        let press_hints = self.tooltip.press_hints;
        let hints = (!press_hints.is_empty()).then(|| self.gesture_hints(press_hints));
        self.footer_row(
            hints,
            closing_delivery_state.map(|delivery_state| self.delivery_state_view(delivery_state)),
        )
    }

    fn delivery_state_view(
        &self,
        delivery_state: SectionsDeliveryState,
    ) -> Element<'static, PopupMessage> {
        match delivery_state {
            SectionsDeliveryState::Delivered { delivery_timing } => {
                self.delivery_timing(delivery_timing)
            }
            SectionsDeliveryState::AwaitingDelivery => row([
                self.small_dim_text("loading".to_owned()).into(),
                Glyph::from(Phosphor::HourglassMedium(Weight::Regular)).view(
                    self.config.theme.font.small_text_size_px,
                    secondary_text_color,
                ),
            ])
            .spacing(GESTURE_ICON_GAP_PX)
            .align_y(Vertical::Center)
            .into(),
        }
    }

    fn footer_row(
        &self,
        hints: Option<Element<'static, PopupMessage>>,
        delivery_timing: Option<Element<'static, PopupMessage>>,
    ) -> Element<'static, PopupMessage> {
        container(
            row(hints
                .into_iter()
                .chain(once(Space::new().width(Length::Fill).into()))
                .chain(delivery_timing))
            .spacing(COLUMN_GAP_PX)
            .align_y(Vertical::Center),
        )
        .padding(self.footer_padding())
        .into()
    }

    fn content_inset(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding::ZERO.horizontal(inset)
    }

    fn shell_padding(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding::ZERO.vertical(inset)
    }

    fn footer_padding(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding {
            left: FOOTER_PADDING.left + inset,
            right: FOOTER_PADDING.right + inset,
            ..FOOTER_PADDING
        }
    }

    fn inset(&self, content: Element<'static, PopupMessage>) -> Element<'static, PopupMessage> {
        container(content).padding(self.content_inset()).into()
    }

    fn gesture_hints(&self, press_hints: PressHints) -> Element<'static, PopupMessage> {
        let icon_size = self.config.theme.font.small_text_size_px;
        row(MouseGesture::ALL.into_iter().filter_map(|gesture| {
            let does = press_hints.does(gesture)?;
            Some(
                row([
                    Glyph::from(gesture.glyph()).view(icon_size, secondary_text_color),
                    self.small_dim_text(does.to_owned()).into(),
                ])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into(),
            )
        }))
        .spacing(COLUMN_GAP_PX)
        .into()
    }

    fn delivery_timing(&self, delivery_timing: DeliveryTiming) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, PopupMessage>| {
            row([label.into(), glyph])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into()
        };
        match delivery_timing {
            DeliveryTiming::PeriodicPolling {
                poll_period,
                delivered_at,
            } => self.refreshable_dial(
                duration_text(poll_period, self.config.calendar.locale),
                poll_period,
                delivered_at,
            ),
            DeliveryTiming::LivePushWithRecheck {
                recheck_period,
                delivered_at,
            } => self.refreshable_dial(
                format!(
                    "live, recheck {}",
                    duration_text(recheck_period, self.config.calendar.locale)
                ),
                recheck_period,
                delivered_at,
            ),
            DeliveryTiming::LivePush => labelled(
                self.small_dim_text("live".to_owned()),
                Glyph::from(Phosphor::Pulse(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
            DeliveryTiming::SingleRead => labelled(
                self.small_dim_text("read once".to_owned()),
                Glyph::from(Phosphor::PushPin(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
        }
    }

    fn refreshable_dial(
        &self,
        label_text: String,
        refresh_period: Duration,
        delivered_at: SystemTime,
    ) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        pressable_text(
            row([
                self.small_dim_text(label_text).into(),
                RefreshDial::new(
                    self.clock
                        .wall
                        .duration_since(delivered_at)
                        .unwrap_or(Duration::ZERO),
                    refresh_period,
                    self.clock.instant,
                    glyph_size,
                )
                .view(),
            ])
            .spacing(GESTURE_ICON_GAP_PX)
            .align_y(Vertical::Center),
            (),
            |()| PopupMessage::RefreshHoveredReading,
            success_shades,
            self.config.tooltips.copy_flash,
            Interaction::Pointer,
        )
    }

    fn small_dim_text(&self, text: String) -> Text<'static> {
        bar_text(&self.config.theme.font, text)
            .size(self.config.theme.font.small_text_size_px)
            .style(text::secondary)
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        bar_text(&self.config.theme.font, title.to_owned())
            .font(FontFaces::from(&self.config.theme.font).bold)
            .style(text::base)
    }

    fn note_title(&self, title: &str) -> Element<'static, PopupMessage> {
        container(self.bold_title(title))
            .max_width(self.config.tooltips.value_width)
            .into()
    }

    fn detailed_note(&self, title: &str, body: &str) -> Element<'static, PopupMessage> {
        Column::with_children([
            self.note_title(title),
            wrapped_text(
                &self.config.theme.font,
                body.to_owned(),
                secondary_text_color,
                self.config.tooltips.value_width,
                Wrapping::default(),
            )
            .into(),
        ])
        .spacing(self.config.layout.overlay.row_spacing)
        .into()
    }

    fn section_seeding(&self) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let line_height_px = LINE_HEIGHT.to_absolute(Pixels(font.text_size_px)).0;
        TransitioningText::new(
            container(Space::new())
                .width(self.config.tooltips.value_width)
                .height(line_height_px),
            String::new(),
            self.transition(TextPresence::Placeholder),
            font,
            secondary_text_color,
        )
        .into()
    }

    fn section(&self, section: &Section) -> Element<'static, PopupMessage> {
        let heading_rows = match section.heading() {
            SectionHeading::Untitled => Vec::new(),
            SectionHeading::Titled(heading) => self.heading_rows(heading),
        };
        self.inset(
            Column::with_children(
                heading_rows
                    .into_iter()
                    .chain(once(self.body(section.body()))),
            )
            .spacing(self.config.layout.overlay.row_spacing)
            .into(),
        )
    }

    fn heading_rows(&self, heading: &Heading) -> Vec<Element<'static, PopupMessage>> {
        let title_padding = Padding::ZERO.left(COLUMN_GAP_PX / 2.0);
        let title = container(self.bold_title(heading.title()))
            .padding(title_padding)
            .into();
        match heading {
            Heading::Plain { .. } => vec![title],
            Heading::Captioned { caption, .. } => vec![
                title,
                container(self.small_dim_text(caption.clone()))
                    .max_width(self.config.tooltips.value_width)
                    .padding(title_padding)
                    .into(),
            ],
        }
    }

    fn body(&self, body: &SectionBody) -> Element<'static, PopupMessage> {
        match body {
            SectionBody::ColumnTable(table) => table.view(&self.record_table_style()),
            SectionBody::CoreGrid(chiplets) => self.core_grid(chiplets),
            SectionBody::ControllerDiagram(diagram) => self.controller_diagram(diagram),
            SectionBody::Tree { root, folding } => {
                ProcessTree::new(root, folding, self.config, self.transition(self.presence)).view()
            }
            SectionBody::Note { title } => self.note_title(title),
            SectionBody::DetailedNote { title, body } => self.detailed_note(title, body),
            SectionBody::SectionSeeding => self.section_seeding(),
            SectionBody::Workspace(preview) => {
                WorkspaceSchematic::new(preview, self.config, self.assets).view()
            }
        }
    }

    fn value_cell_view(&self) -> ValueCellView<PopupMessage> {
        ValueCellView {
            figure_font: self.config.theme.monospace_font.clone(),
            prose_font: self.config.theme.font.clone(),
            max_width: self.config.tooltips.value_width,
            wrapping: Wrapping::default(),
            locale: self.config.calendar.locale,
            now: self.clock.utc(),
            flash: success_shades,
            link: primary_shades,
            hint: secondary_text_color,
            transition: self.transition(self.presence),
            copy_flash: self.config.tooltips.copy_flash,
            press_messages: ValueCellPressMessages {
                copy_text: PopupMessage::CopyText,
                open_url: PopupMessage::OpenUrl,
            },
            font_style: FontStyle::Normal,
        }
    }

    fn record_table_style(&self) -> KeyedRecordTableStyle<PopupMessage> {
        KeyedRecordTableStyle {
            cell_view: self.value_cell_view(),
            key_color: text_color,
            heading_font: self.config.theme.font.clone(),
            column_gap: COLUMN_GAP_PX,
            row_spacing: self.config.layout.overlay.row_spacing,
        }
    }

    fn controller_diagram(&self, diagram: &ControllerCallouts) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let row_height = LINE_HEIGHT.to_absolute(font.text_size_px.into()).0.ceil();
        diagram
            .callouts
            .iter()
            .fold(
                controller_diagram(self.assets.controller_artwork.clone(), diagram.model)
                    .body_width(CONTROLLER_DIAGRAM_WIDTH_PX)
                    .row_height(row_height)
                    .style(|theme| DiagramStyle {
                        line: secondary_text_color(theme),
                        anchor: primary_color(theme),
                    }),
                |drawn, callout| {
                    drawn.callout(
                        callout.anchor,
                        bar_text(font, callout.label.clone())
                            .style(WidgetState::from(callout.level).text_style()),
                    )
                },
            )
            .into()
    }

    fn core_grid(&self, chiplets: &[ChipletCells]) -> Element<'static, PopupMessage> {
        let spacing = self.config.layout.overlay.row_spacing;
        Column::with_children(chiplets.iter().map(|chiplet| {
            let cells = container(CoreCellGrid::new(&chiplet.cores, self.config).view())
                .padding(Padding::ZERO.left(COLUMN_GAP_PX / 2.0));
            Column::with_children([
                container(
                    bar_text(&self.config.theme.font, chiplet.title.clone()).style(text::base),
                )
                .padding(Padding::ZERO.left(COLUMN_GAP_PX / 2.0))
                .into(),
                row![
                    cells,
                    KeyedRecordTable::labeled_values(chiplet.facts.clone())
                        .view(&self.record_table_style())
                ]
                .spacing(COLUMN_GAP_PX)
                .align_y(Vertical::Center)
                .into(),
            ])
            .spacing(spacing / 2.0)
            .into()
        }))
        .spacing(spacing)
        .into()
    }
}
