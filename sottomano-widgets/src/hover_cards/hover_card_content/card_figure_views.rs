use std::rc::Rc;

use corredo::{
    data_tables::GroupedTableStyle,
    theming::{
        LINE_HEIGHT, background_strong_color, primary_color, primary_shades, secondary_text_color,
        success_shades,
    },
};
use iced::{
    Element,
    alignment::Vertical,
    font::Style as FontStyle,
    widget::{Column, container, row, text, text::Wrapping},
};
use iced_controller_diagram::{ControllerDiagram, Style as DiagramStyle};
use sottomano_desktop_sources::desktop_windows_apps::process_tree_inspection::Process;
use sottomano_toml_config::severity::Severity;

use super::{HoverCardContent, VALUE_COLUMN_WIDTH_PX};
use crate::{
    group_indicators::indicator_state::IndicatorState,
    hover_cards::{
        PopupMessage,
        core_grid::CoreCellGrid,
        hover_card_body::{
            dictionary_table,
            hover_card_figure_payloads::{ChipletCells, ControllerCallouts, WorkspacePreview},
        },
    },
    output_windows::process_tree::{Folding, ProcessTree},
    widgets::{
        text::{column_heading, panel_text},
        value_cells::{ValueCell, ValueCellPressMessages, ValueCellView},
    },
    workspace_switcher::workspace_preview::WorkspaceSchematic,
};

const CONTROLLER_DIAGRAM_WIDTH_PX: f32 = 300.0;

impl HoverCardContent<'_> {
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
