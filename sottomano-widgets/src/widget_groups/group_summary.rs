use corredo::{orientation::Axis, transitions::reveal};
use iced::{
    Alignment, Element, Padding,
    widget::{Row, container},
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item::{GroupItem, GroupItemValue, MenuParts},
        reading_display_state::ReadingDisplayState,
        slider_control::ShownReading,
        source_reading::{GroupItemIcon, reading_value},
    },
    widget_groups::widget_group_content::GroupEvent,
    widgets::icon_glyph::IconGlyph,
};

enum SummaryValue {
    Hidden,
    Shown(ShownReading),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SummaryVisibility {
    GlyphOnly,
    WithReading,
}

pub struct GroupSummary<'a, M> {
    instrument: Box<GroupItem<'a, M>>,
    value: SummaryValue,
}

impl<'a, M: Clone + 'a> GroupSummary<'a, M> {
    #[must_use]
    pub fn glyph(glyph: IconGlyph, state: ReadingDisplayState) -> Self {
        Self {
            instrument: Box::new(GroupItem::composed(
                "",
                GroupItemIcon::IconGlyph { glyph, state },
                GroupItemValue::Nothing,
                Box::new(|_| Vec::new()),
            )),
            value: SummaryValue::Hidden,
        }
    }

    #[must_use]
    pub fn showing(glyph: IconGlyph, row: &GroupItem<'a, M>) -> Self {
        let instrument = Self::glyph(glyph, row.display_state())
            .instrument
            .sharing_details_of(row);
        let value = match row.shown_reading() {
            Some(shown_reading) => SummaryValue::Shown(shown_reading),
            None => SummaryValue::Hidden,
        };
        Self {
            instrument: Box::new(instrument),
            value,
        }
    }

    #[must_use]
    pub fn adjusting_instrument(
        self,
        adjust: impl FnOnce(GroupItem<'a, M>) -> GroupItem<'a, M>,
    ) -> Self {
        Self {
            instrument: Box::new(adjust(*self.instrument)),
            ..self
        }
    }

    pub(crate) fn shown_value(&self) -> SummaryVisibility {
        match self.value {
            SummaryValue::Hidden => SummaryVisibility::GlyphOnly,
            SummaryValue::Shown(_) => SummaryVisibility::WithReading,
        }
    }

    pub(crate) fn instrument_item(&self) -> &GroupItem<'a, M> {
        &self.instrument
    }

    pub(crate) fn handle_view(
        &self,
        config: &'a Config,
        reveal_fraction: f32,
        clock: Clock,
    ) -> Element<'a, GroupEvent<M>> {
        Row::with_children([
            self.instrument.unhovered_view(config, 1.0, clock),
            self.collapsing_value(config, reveal_fraction),
        ])
        .align_y(Alignment::Center)
        .into()
    }

    pub(crate) fn menu_parts(&self, config: &'a Config, clock: Clock) -> MenuParts<'a, M> {
        let instrument_parts = self.instrument.menu_parts(config, clock);
        match self.value {
            SummaryValue::Hidden => instrument_parts,
            SummaryValue::Shown(_) => MenuParts {
                icon: instrument_parts.icon,
                value: self.collapsing_value(config, 0.0),
            },
        }
    }

    fn collapsing_value(
        &self,
        config: &Config,
        reveal_fraction: f32,
    ) -> Element<'a, GroupEvent<M>> {
        let hidden = 1.0 - reveal_fraction;
        let SummaryValue::Shown(shown_reading) = &self.value else {
            return Row::new().into();
        };
        let Some(value) = reading_value(config, &shown_reading.face, &shown_reading.readout, 1.0)
            .filter(|_| hidden > 0.0)
        else {
            return Row::new().into();
        };
        reveal(
            container(value).padding(Padding::ZERO.left(config.theme.spacing.item_gap)),
            hidden,
            Axis::Horizontal,
        )
        .into()
    }

    pub(crate) fn map<Outer: Clone + 'a>(self, lift: fn(M) -> Outer) -> GroupSummary<'a, Outer> {
        GroupSummary {
            instrument: Box::new(self.instrument.map(lift)),
            value: self.value,
        }
    }
}
