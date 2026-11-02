use std::{
    iter::{empty, once},
    time::{Duration, SystemTime},
};

use iced::{
    Background, Element, Length, Padding, Pixels,
    alignment::Vertical,
    font::Style as FontStyle,
    mouse::Interaction,
    widget::{Column, Space, Text, button, container, row, text, text::Wrapping},
};
use iced_controller_diagram::{Style as DiagramStyle, controller_diagram};
use sottomano_config::Config;

use crate::{
    clock::Clock,
    hover_tooltips::{
        HoverOrigin, MouseGesture, PanelPin, PopupMessage, PressHints, core_grid::CoreCellGrid,
    },
    widget_groups::{
        group_menus::PinnedEntry, window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        bar_widget::Glyph,
        configured_theme::{
            primary_color, primary_shades, primary_weak_color, secondary_text_color, success_color,
            success_shades, text_color,
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

const CONTROLLER_DIAGRAM_WIDTH_PX: f32 = 300.0;

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
        let group_title = self.group_title_row().map(lift);
        let groups: Vec<&SourcedSections> = self.tooltip.filled_groups().collect();
        let Some((closing_group, leading_groups)) = groups.split_last() else {
            let content = uniform_width_column(
                enclosed_panel
                    .into_iter()
                    .chain(once(lift(self.closing_row(None)))),
            )
            .into();
            return self.shell_below_title(group_title, content);
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
        .spacing(self.config.layout.overlay.row_spacing)
        .into();
        let content = uniform_width_column(leading.chain(once(closing)))
            .spacing(self.config.layout.overlay.section_spacing)
            .into();
        self.shell_below_title(group_title, content)
    }

    fn shell_below_title<'panel, Message: 'panel>(
        &self,
        group_title: Option<Element<'panel, Message>>,
        content: Element<'panel, Message>,
    ) -> Element<'panel, Message> {
        container(
            uniform_width_column(group_title.into_iter().chain(once(content)))
                .spacing(self.config.layout.overlay.row_spacing),
        )
        .padding(self.shell_padding())
        .into()
    }

    fn group_title_row(&self) -> Option<Element<'static, PopupMessage>> {
        match &self.tooltip.announcement.origin {
            HoverOrigin::GroupHandle {
                group_title,
                panel_pin,
            } => {
                let title: Element<'static, PopupMessage> =
                    container(self.small_dim_text(group_title.to_string()))
                        .padding(self.edge_row_padding())
                        .into();
                let unpin = match *panel_pin {
                    PanelPin::Unpinned => None,
                    PanelPin::GroupPinnedFrom(menu) => {
                        Some(self.unpin_action_row(PopupMessage::ToggleGroupMenuPin {
                            menu,
                            entry: PinnedEntry::Group {
                                group_title: (**group_title).clone(),
                            },
                        }))
                    }
                    PanelPin::ItemPinnedFrom { menu, item_name } => {
                        Some(self.unpin_action_row(PopupMessage::ToggleGroupMenuPin {
                            menu,
                            entry: PinnedEntry::Item {
                                group_title: (**group_title).clone(),
                                item_name,
                            },
                        }))
                    }
                };
                Some(
                    Column::with_children(once(title).chain(unpin))
                        .spacing(self.config.layout.overlay.row_spacing)
                        .into(),
                )
            }
            HoverOrigin::Reading => None,
        }
    }

    fn unpin_action_row(&self, unpin: PopupMessage) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let label = row([
            Glyph::from(Phosphor::PushPin(Weight::Fill)).view(font.icon_size_px, text_color),
            bar_text(font, "unpin from the panel").into(),
        ])
        .spacing(self.config.layout.overlay.row_spacing)
        .align_y(Vertical::Center);
        button(label)
            .width(Length::Fill)
            .padding(
                self.edge_row_padding()
                    .vertical(self.config.layout.overlay.row_spacing),
            )
            .style(|theme, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then(|| Background::Color(primary_weak_color(theme))),
                text_color: text_color(theme),
                ..button::Style::default()
            })
            .on_press(unpin)
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
        .spacing(self.config.layout.overlay.section_spacing)
        .into();
        uniform_width_column(source_heading.into_iter().chain(once(sections)))
            .spacing(self.config.layout.overlay.row_spacing)
            .into()
    }

    fn closing_row(
        &self,
        closing_delivery_state: Option<SectionsDeliveryState>,
    ) -> Element<'static, PopupMessage> {
        let press_hints = self.tooltip.announcement.press_hints;
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
            .spacing(self.config.layout.overlay.row_spacing)
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
            .spacing(self.config.layout.overlay.column_spacing)
            .align_y(Vertical::Center),
        )
        .padding(self.edge_row_padding())
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

    fn edge_row_padding(&self) -> Padding {
        let overlay = &self.config.layout.overlay;
        Padding::ZERO
            .left(overlay.content_inset + overlay.row_spacing)
            .right(overlay.content_inset)
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
                .spacing(self.config.layout.overlay.row_spacing)
                .align_y(Vertical::Center)
                .into(),
            )
        }))
        .spacing(self.config.layout.overlay.column_spacing)
        .into()
    }

    fn delivery_timing(&self, delivery_timing: DeliveryTiming) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, PopupMessage>| {
            row([label.into(), glyph])
                .spacing(self.config.layout.overlay.row_spacing)
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
            .spacing(self.config.layout.overlay.row_spacing)
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
        let title_padding = Padding::ZERO.left(self.config.layout.overlay.row_spacing);
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
            column_gap: self.config.layout.overlay.column_spacing,
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
                .padding(Padding::ZERO.left(self.config.layout.overlay.row_spacing));
            Column::with_children([
                container(
                    bar_text(&self.config.theme.font, chiplet.title.clone()).style(text::base),
                )
                .padding(Padding::ZERO.left(self.config.layout.overlay.row_spacing))
                .into(),
                row![
                    cells,
                    KeyedRecordTable::labeled_values(chiplet.facts.clone())
                        .view(&self.record_table_style())
                ]
                .spacing(self.config.layout.overlay.column_spacing)
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
