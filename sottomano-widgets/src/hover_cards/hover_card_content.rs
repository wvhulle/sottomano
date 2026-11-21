use std::{
    iter::once,
    rc::Rc,
    time::{Duration, Instant},
};

use corredo::{
    changing_text::{TextPresence, TransitionStyle, TransitioningText},
    data_tables::GroupedTableStyle,
    gauges::CountdownDial,
    orientation::Axis,
    press_actions::flash_on_press_text,
    selection_scroll::ScrollEdgeFade,
    theming::{
        FrameStyle, LINE_HEIGHT, OUTLINE_WIDTH_PX, background_strong_color, danger_color,
        primary_color, primary_shades, primary_weak_color, secondary_text_color, success_color,
        success_shades, text_color,
    },
    width_layout::equal_width_column,
};
use iced::{
    Background, Element, Length, Padding, Pixels,
    alignment::Vertical,
    font::Style as FontStyle,
    mouse::Interaction,
    widget::{Column, Space, Text, button, container, row, rule, text, text::Wrapping},
};
use iced_controller_diagram::{ControllerDiagram, Style as DiagramStyle};
use sottomano_desktop_sources::{
    delivery_cadence_drivers::DeliveryCadence,
    desktop_windows_apps::process_tree_inspection::Process,
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::indicator_state::IndicatorState,
    group_menus::{
        group_menu_state::{GroupMenuPin, PanelPin, PanelPinDuration},
        menu_rows::menu_row::menu_metrics,
    },
    hover_cards::{
        CardHeader, PopupMessage, PressAction,
        core_grid::CoreCellGrid,
        hover_card_body::{
            Section, SectionTitle, dictionary_table,
            hover_card_figure_payloads::{ChipletCells, ControllerCallouts, WorkspacePreview},
            hover_card_source_delivery::{DeliveryStamp, HoverCard, SectionSource, SourceDelivery},
        },
    },
    icon_catalog::window_icons::WindowAssets,
    indicator_groups::{
        window_group::process_tree::{Folding, ProcessTree},
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        configured_scroll::configured_scroll,
        localized_durations::duration_text,
        text::{column_heading, font_faces_of, panel_text, text_effects_of, text_face_of},
        value_cells::{ValueCell, ValueCellPressMessages, ValueCellView, wrapped_text},
    },
};

const CONTROLLER_DIAGRAM_WIDTH_PX: f32 = 300.0;
const VALUE_COLUMN_WIDTH_PX: f32 = 620.0;

#[derive(Clone, Copy)]
pub struct HoverCardContent<'a> {
    hover_card: &'a HoverCard,
    config: &'a Config,
    assets: WindowAssets<'a>,
    clock: Clock,
    presence: TextPresence,
}

impl<'a> HoverCardContent<'a> {
    #[must_use]
    pub fn new(
        hover_card: &'a HoverCard,
        config: &'a Config,
        assets: WindowAssets<'a>,
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
            effects: text_effects_of(self.config.hover_cards.loading),
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
                self.section(section, source_row)
            })
            .collect();
        let content = equal_width_column(section_views)
            .spacing(self.config.theme.spacing.group_gap)
            .into();
        let scroll_fade_extent_px =
            self.config.group_menus.scroll_fade_rows * menu_metrics(self.config).row_height_px;
        configured_scroll(
            self.shell_below_title(title_rows, content),
            Axis::Vertical,
            self.config,
        )
        .edge_fade(ScrollEdgeFade::within_frame(frame, scroll_fade_extent_px))
        .into()
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

    fn card_header_rows(&self) -> Option<Element<'static, PopupMessage>> {
        let CardHeader::Pinned {
            group_title,
            pin: panel_pin,
        } = &self.hover_card.header
        else {
            return None;
        };
        let group_name = self.small_dim_text(group_title.to_string());
        let header: Element<'static, PopupMessage> = match &panel_pin.entry {
            GroupMenuPin::Group(_) => group_name.into(),
            GroupMenuPin::Item(indicator_key) => row([
                group_name.into(),
                self.small_dim_text("\u{203a}".to_owned()).into(),
                self.bold_title(&indicator_key.name).into(),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center)
            .into(),
        };
        let unpin = self.release_action_row((**panel_pin).clone());
        let header_row: Element<'static, PopupMessage> =
            container(container(header).padding(self.title_padding()))
                .padding(self.edge_row_padding())
                .into();
        Some(
            Column::with_children([header_row, Self::separator_rule(), unpin])
                .spacing(self.config.theme.spacing.item_gap)
                .into(),
        )
    }

    fn separator_rule() -> Element<'static, PopupMessage> {
        rule::horizontal(OUTLINE_WIDTH_PX)
            .style(rule::default)
            .into()
    }

    fn release_action_row(&self, panel_pin: PanelPin) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let (release_glyph, release_text) = match panel_pin.duration {
            PanelPinDuration::Permanent => {
                (Phosphor::PushPin(Weight::Fill), "unpin from the panel")
            }
            PanelPinDuration::UntilAlertEnds => (
                Phosphor::PushPinSlash(Weight::Regular),
                "temporarily pinned while alerting: dismiss until it alerts again",
            ),
        };
        let label = row([
            IconGlyph::from(release_glyph).view(font.icon_size_px, text_color),
            panel_text(font, release_text).into(),
        ])
        .spacing(self.config.theme.spacing.item_gap)
        .align_y(Vertical::Center);
        button(container(label).padding(self.title_padding()))
            .width(Length::Fill)
            .padding(
                self.edge_row_padding()
                    .vertical(self.config.theme.spacing.item_gap),
            )
            .style(|theme, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then(|| Background::Color(primary_weak_color(theme))),
                text_color: text_color(theme),
                ..button::Style::default()
            })
            .on_press(PopupMessage::ReleaseFromPanel(panel_pin))
            .into()
    }

    fn source_row(
        &self,
        source_name: &'static str,
        delivery: SourceDelivery,
    ) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        container(
            row([
                IconGlyph::from(Phosphor::Database(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
                self.small_dim_text(source_name.to_owned()).into(),
                Space::new().width(Length::Fill).into(),
                self.delivery_view(delivery),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center),
        )
        .padding(self.title_padding())
        .into()
    }

    fn delivery_view(&self, delivery: SourceDelivery) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>,
                        glyph: Phosphor,
                        glyph_color: fn(&iced::Theme) -> iced::Color| {
            row([
                label.into(),
                IconGlyph::from(glyph).view(glyph_size, glyph_color),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center)
            .into()
        };
        match delivery {
            SourceDelivery::Delivered(delivery_stamp) => self.delivery_stamp(delivery_stamp),
            SourceDelivery::Loading => labelled(
                self.small_dim_text("loading".to_owned()),
                Phosphor::HourglassMedium(Weight::Regular),
                secondary_text_color,
            ),
            SourceDelivery::Failed => labelled(
                self.small_text("failed".to_owned())
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(danger_color(theme)),
                    }),
                Phosphor::Warning(Weight::Regular),
                danger_color,
            ),
        }
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
            text_face_of(font),
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

    fn delivery_stamp(&self, delivery_stamp: DeliveryStamp) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, PopupMessage>| {
            row([label.into(), glyph])
                .spacing(self.config.theme.spacing.item_gap)
                .align_y(Vertical::Center)
                .into()
        };
        let locale = self.config.calendar.locale;
        match delivery_stamp.cadence {
            DeliveryCadence::PeriodicPolling(polling) => self.refreshable_dial(
                duration_text(polling.poll_period, locale),
                polling.poll_period,
                delivery_stamp.delivered_at,
            ),
            DeliveryCadence::LivePushWithRecheck(polling) => self.refreshable_dial(
                format!(
                    "live, recheck {}",
                    duration_text(polling.poll_period, locale)
                ),
                polling.poll_period,
                delivery_stamp.delivered_at,
            ),
            DeliveryCadence::LivePush => labelled(
                self.small_dim_text("live".to_owned()),
                IconGlyph::from(Phosphor::Pulse(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
            DeliveryCadence::SingleRead => labelled(
                self.small_dim_text("read once".to_owned()),
                IconGlyph::from(Phosphor::PushPin(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
        }
    }

    fn refreshable_dial(
        &self,
        label_text: String,
        refresh_period: Duration,
        delivered_at: Instant,
    ) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        Element::from(
            flash_on_press_text(
                row([
                    self.small_dim_text(label_text).into(),
                    CountdownDial::new(
                        self.clock
                            .instant
                            .checked_duration_since(delivered_at)
                            .expect("a delivery is stamped no later than the clock that reads it"),
                        refresh_period,
                        self.clock.instant,
                        glyph_size,
                    )
                    .view(),
                ])
                .spacing(self.config.theme.spacing.item_gap)
                .align_y(Vertical::Center),
                (),
                |()| PopupMessage::RefreshHoveredIndicator,
            )
            .accent(success_shades)
            .hold(self.config.hover_cards.copy_flash_duration)
            .interaction(Interaction::Pointer),
        )
    }

    fn small_text(&self, text: String) -> Text<'static> {
        panel_text(&self.config.theme.font, text).size(self.config.theme.font.small_text_size_px)
    }

    fn small_dim_text(&self, text: String) -> Text<'static> {
        self.small_text(text).style(text::secondary)
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        panel_text(&self.config.theme.font, title.to_owned())
            .font(font_faces_of(&self.config.theme.font).bold)
            .style(text::base)
    }

    fn section(
        &self,
        section: &Section,
        source_row: Option<Element<'static, PopupMessage>>,
    ) -> Element<'static, PopupMessage> {
        let delivered = match section.source {
            SectionSource::Snapshot { delivery, .. } => self.in_delivery_state(delivery),
            SectionSource::Local => *self,
        };
        let section_view = (section.widget.draw)(&delivered);
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

    pub(crate) fn workspace_schematic(
        &self,
        preview: &WorkspacePreview,
    ) -> Element<'static, PopupMessage> {
        WorkspaceSchematic::new(preview, self.config, self.assets).view()
    }

    pub(crate) fn process_tree(
        &self,
        root: &Process,
        folding: &Folding,
    ) -> Element<'static, PopupMessage> {
        ProcessTree::new(root, folding, self.config, self.transition(self.presence)).view()
    }

    fn value_cell_view(&self) -> ValueCellView<PopupMessage> {
        ValueCellView {
            figure_font: self.config.theme.monospace_font.clone(),
            prose_font: self.config.theme.font.clone(),
            max_width: VALUE_COLUMN_WIDTH_PX,
            glyph_gap: self.config.theme.spacing.item_gap,
            wrapping: Wrapping::WordOrGlyph,
            locale: self.config.calendar.locale,
            now: self.clock.utc(),
            flash: success_shades,
            link: primary_shades,
            hint: secondary_text_color,
            transition: self.transition(self.presence),
            copy_flash: self.config.hover_cards.copy_flash_duration,
            press_messages: ValueCellPressMessages {
                copy_text: PopupMessage::CopyText,
                open_url: PopupMessage::OpenUrl,
            },
            font_style: FontStyle::Normal,
        }
    }

    pub(crate) fn record_table_style(
        &self,
    ) -> GroupedTableStyle<ValueCell, Severity, PopupMessage> {
        let key_view = self.value_cell_view().key_view();
        let omitted_key_view = key_view.clone();
        let cell_view = self.value_cell_view();
        let heading_font = self.config.theme.font.clone();
        GroupedTableStyle {
            key_cell: Rc::new(move |record| key_view.key(record.key.clone(), secondary_text_color)),
            value_cell: Rc::new(move |record, position| {
                let state = IndicatorState::from(record.level);
                cell_view.value(
                    record
                        .cells
                        .get(position)
                        .cloned()
                        .unwrap_or(ValueCell::Blank),
                    move |theme| state.color(theme),
                    &record.key,
                )
            }),
            omitted_count: Rc::new(move |omitted_count| {
                omitted_key_view.key(
                    ValueCell::from(format!("{omitted_count} more")),
                    background_strong_color,
                )
            }),
            heading: Rc::new(move |label| column_heading(&heading_font, label.to_owned()).into()),
            text_size_px: self.config.theme.font.text_size_px,
            column_gap_px: self.config.theme.spacing.group_gap,
            row_spacing_px: self.config.theme.spacing.item_gap,
        }
    }

    pub(crate) fn controller_diagram(
        &self,
        diagram: &ControllerCallouts,
    ) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let row_height = LINE_HEIGHT.to_absolute(font.text_size_px.into()).0.ceil();
        diagram
            .callouts
            .iter()
            .fold(
                ControllerDiagram::new(self.assets.controller_artwork.clone(), diagram.model)
                    .body_width(CONTROLLER_DIAGRAM_WIDTH_PX)
                    .row_height(row_height)
                    .style(|theme| DiagramStyle {
                        line: secondary_text_color(theme),
                        anchor: primary_color(theme),
                    }),
                |drawn, callout| {
                    drawn.callout(
                        callout.anchor,
                        panel_text(font, callout.label.clone())
                            .style(IndicatorState::from(callout.level).text_style()),
                    )
                },
            )
            .into()
    }

    pub(crate) fn core_grid(&self, chiplets: &[ChipletCells]) -> Element<'static, PopupMessage> {
        let spacing = self.config.theme.spacing.item_gap;
        Column::with_children(chiplets.iter().map(|chiplet| {
            let cells = container(CoreCellGrid::new(&chiplet.cores, self.config).view())
                .padding(self.title_padding());
            Column::with_children([
                container(
                    panel_text(&self.config.theme.font, chiplet.title.clone()).style(text::base),
                )
                .padding(self.title_padding())
                .into(),
                row![
                    cells,
                    dictionary_table(chiplet.facts.clone()).view(&self.record_table_style())
                ]
                .spacing(self.config.theme.spacing.group_gap)
                .align_y(Vertical::Center)
                .into(),
            ])
            .spacing(spacing)
            .into()
        }))
        .spacing(spacing)
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
