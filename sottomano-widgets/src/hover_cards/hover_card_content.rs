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
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::reading_display_state::ReadingDisplayState,
    group_menus::group_menu_state::GroupMenuPin,
    hover_cards::{
        HoverOrigin, MouseGesture, PanelPinOrigin, PopupMessage, PressHints,
        core_grid::CoreCellGrid,
        hover_card_body::{
            Heading, Section, SectionBody, SectionHeading,
            hover_card_figure_payloads::{ChipletCells, ControllerCallouts},
            hover_card_source_delivery::{
                DeliveryTiming, HoverCard, SectionsDeliveryState, SourcedSections,
            },
        },
    },
    widget_groups::{
        window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        configured_theme::{
            primary_color, primary_shades, primary_weak_color, secondary_text_color, success_color,
            success_shades, text_color,
        },
        icon_glyph::IconGlyph,
        keyed_record_table::{KeyedRecordTable, KeyedRecordTableStyle},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        pressable_text::pressable_text,
        refresh_dial::RefreshDial,
        text::{FontFaces, LINE_HEIGHT, panel_text},
        transitioning_text::{TextPresence, TransitionStyle, TransitioningText},
        uniform_width_column::uniform_width_column,
        value_cells::{ValueCellPressMessages, ValueCellView, wrapped_text},
        window_icon::WindowAssets,
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

    fn in_delivery_state(self, delivery_state: SectionsDeliveryState) -> Self {
        let presence = match delivery_state {
            SectionsDeliveryState::Delivered { .. } => TextPresence::Shown,
            SectionsDeliveryState::AwaitingDelivery => TextPresence::Placeholder,
        };
        Self { presence, ..self }
    }

    fn transition(&self, presence: TextPresence) -> TransitionStyle {
        TransitionStyle {
            loading: self.config.hover_cards.loading,
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
        let groups: Vec<&SourcedSections> = self.hover_card.filled_groups().collect();
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
                .spacing(self.config.theme.spacing.item_gap)
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
        .spacing(self.config.theme.spacing.item_gap)
        .into();
        let content = uniform_width_column(leading.chain(once(closing)))
            .spacing(self.config.theme.spacing.group_gap)
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
                .spacing(self.config.theme.spacing.item_gap),
        )
        .padding(self.shell_padding())
        .into()
    }

    fn group_title_row(&self) -> Option<Element<'static, PopupMessage>> {
        match &self.hover_card.announcement.origin {
            HoverOrigin::GroupHandle {
                group_title,
                panel_pin,
            } => {
                let title: Element<'static, PopupMessage> = container(
                    container(self.small_dim_text(group_title.to_string()))
                        .padding(self.title_padding()),
                )
                .padding(self.edge_row_padding())
                .into();
                let unpin = match panel_pin {
                    PanelPinOrigin::Unpinned => None,
                    PanelPinOrigin::GroupPinnedFrom { menu, group } => {
                        Some(self.unpin_action_row(PopupMessage::ToggleGroupMenuPin {
                            menu: *menu,
                            entry: GroupMenuPin::Group((**group).clone()),
                        }))
                    }
                    PanelPinOrigin::ItemPinnedFrom {
                        menu,
                        group,
                        item_key,
                    } => Some(self.unpin_action_row(PopupMessage::ToggleGroupMenuPin {
                        menu: *menu,
                        entry: GroupMenuPin::Item {
                            group: (**group).clone(),
                            item_key: item_key.clone(),
                        },
                    })),
                };
                Some(
                    Column::with_children(once(title).chain(unpin))
                        .spacing(self.config.theme.spacing.item_gap)
                        .into(),
                )
            }
            HoverOrigin::Reading => None,
        }
    }

    fn unpin_action_row(&self, unpin: PopupMessage) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let label = row([
            IconGlyph::from(Phosphor::PushPin(Weight::Fill)).view(font.icon_size_px, text_color),
            panel_text(font, "unpin from the panel").into(),
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
            .on_press(unpin)
            .into()
    }

    fn group_sections(&self, group: &SourcedSections) -> Element<'static, PopupMessage> {
        let content = self.in_delivery_state(group.delivery_state());
        let source_heading = group.source_heading().map(|heading| {
            self.inset(
                Column::with_children(self.heading_rows(heading))
                    .spacing(self.config.theme.spacing.item_gap)
                    .into(),
            )
        });
        let sections: Element<'static, PopupMessage> = uniform_width_column(
            group
                .filled_sections()
                .map(|section| content.section(section)),
        )
        .spacing(self.config.theme.spacing.group_gap)
        .into();
        uniform_width_column(source_heading.into_iter().chain(once(sections)))
            .spacing(self.config.theme.spacing.item_gap)
            .into()
    }

    fn closing_row(
        &self,
        closing_delivery_state: Option<SectionsDeliveryState>,
    ) -> Element<'static, PopupMessage> {
        let press_hints = self.hover_card.announcement.press_hints;
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
                IconGlyph::from(Phosphor::HourglassMedium(Weight::Regular)).view(
                    self.config.theme.font.small_text_size_px,
                    secondary_text_color,
                ),
            ])
            .spacing(self.config.theme.spacing.item_gap)
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
            container(
                row(hints
                    .into_iter()
                    .chain(once(Space::new().width(Length::Fill).into()))
                    .chain(delivery_timing))
                .spacing(self.config.theme.spacing.group_gap)
                .align_y(Vertical::Center),
            )
            .padding(self.title_padding()),
        )
        .padding(self.edge_row_padding())
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

    fn gesture_hints(&self, press_hints: PressHints) -> Element<'static, PopupMessage> {
        let icon_size = self.config.theme.font.small_text_size_px;
        row(MouseGesture::ALL.into_iter().filter_map(|gesture| {
            let does = press_hints.does(gesture)?;
            Some(
                row([
                    IconGlyph::from(gesture.glyph()).view(icon_size, secondary_text_color),
                    self.small_dim_text(does.to_owned()).into(),
                ])
                .spacing(self.config.theme.spacing.item_gap)
                .align_y(Vertical::Center)
                .into(),
            )
        }))
        .spacing(self.config.theme.spacing.group_gap)
        .into()
    }

    fn delivery_timing(&self, delivery_timing: DeliveryTiming) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, PopupMessage>| {
            row([label.into(), glyph])
                .spacing(self.config.theme.spacing.item_gap)
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
                IconGlyph::from(Phosphor::Pulse(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
            DeliveryTiming::SingleRead => labelled(
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
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center),
            (),
            |()| PopupMessage::RefreshHoveredReading,
            success_shades,
            self.config.hover_cards.copy_flash_duration,
            Interaction::Pointer,
        )
    }

    fn small_dim_text(&self, text: String) -> Text<'static> {
        panel_text(&self.config.theme.font, text)
            .size(self.config.theme.font.small_text_size_px)
            .style(text::secondary)
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        panel_text(&self.config.theme.font, title.to_owned())
            .font(FontFaces::from(&self.config.theme.font).bold)
            .style(text::base)
    }

    fn note_title(&self, title: &str) -> Element<'static, PopupMessage> {
        container(self.bold_title(title))
            .max_width(VALUE_COLUMN_WIDTH_PX)
            .into()
    }

    fn detailed_note(&self, title: &str, body: &str) -> Element<'static, PopupMessage> {
        Column::with_children([
            self.note_title(title),
            wrapped_text(
                &self.config.theme.font,
                body.to_owned(),
                secondary_text_color,
                VALUE_COLUMN_WIDTH_PX,
                Wrapping::default(),
            )
            .into(),
        ])
        .spacing(self.config.theme.spacing.item_gap)
        .into()
    }

    fn section_seeding(&self) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let line_height_px = LINE_HEIGHT.to_absolute(Pixels(font.text_size_px)).0;
        TransitioningText::new(
            container(Space::new())
                .width(VALUE_COLUMN_WIDTH_PX)
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
            .spacing(self.config.theme.spacing.item_gap)
            .into(),
        )
    }

    fn heading_rows(&self, heading: &Heading) -> Vec<Element<'static, PopupMessage>> {
        let title = container(self.bold_title(heading.title()))
            .padding(self.title_padding())
            .into();
        match heading {
            Heading::Plain { .. } => vec![title],
            Heading::Captioned { caption, .. } => vec![
                title,
                container(self.small_dim_text(caption.clone()))
                    .max_width(VALUE_COLUMN_WIDTH_PX)
                    .padding(self.title_padding())
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
            max_width: VALUE_COLUMN_WIDTH_PX,
            glyph_gap: self.config.theme.spacing.item_gap,
            wrapping: Wrapping::default(),
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

    fn record_table_style(&self) -> KeyedRecordTableStyle<PopupMessage> {
        KeyedRecordTableStyle {
            cell_view: self.value_cell_view(),
            key_color: text_color,
            heading_font: self.config.theme.font.clone(),
            column_gap: self.config.theme.spacing.group_gap,
            row_spacing: self.config.theme.spacing.item_gap,
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
                        panel_text(font, callout.label.clone())
                            .style(ReadingDisplayState::from(callout.level).text_style()),
                    )
                },
            )
            .into()
    }

    fn core_grid(&self, chiplets: &[ChipletCells]) -> Element<'static, PopupMessage> {
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
                    KeyedRecordTable::labeled_values(chiplet.facts.clone())
                        .view(&self.record_table_style())
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
