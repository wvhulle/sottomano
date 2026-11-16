use corredo::{orientation::Axis, transitions::reveal};
use iced::{
    Alignment, Element, Padding,
    widget::{Row, Space, container},
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem, reading_display_state::ReadingDisplayState,
        source_reading::GroupItemIcon,
    },
    widget_groups::widget_group_content::GroupEvent,
    widgets::icon_glyph::IconGlyph,
};

pub enum GroupSummary<'a> {
    Glyph {
        glyph: IconGlyph,
        state: ReadingDisplayState,
    },
    Row {
        glyph: IconGlyph,
        row: Box<GroupItem<'a>>,
    },
}

impl<'a> GroupSummary<'a> {
    #[must_use]
    pub fn glyph(glyph: IconGlyph, state: ReadingDisplayState) -> Self {
        GroupSummary::Glyph { glyph, state }
    }

    #[must_use]
    pub fn summarising(glyph: IconGlyph, row: GroupItem<'a>) -> Self {
        GroupSummary::Row {
            glyph,
            row: Box::new(row),
        }
    }

    pub(crate) fn summarised_row(&self) -> Option<&GroupItem<'a>> {
        match self {
            GroupSummary::Glyph { .. } => None,
            GroupSummary::Row { row, .. } => Some(row),
        }
    }

    fn glyph_icon(&self) -> GroupItemIcon {
        match self {
            GroupSummary::Glyph { glyph, state } => GroupItemIcon::IconGlyph {
                glyph: *glyph,
                state: *state,
            },
            GroupSummary::Row { glyph, row } => GroupItemIcon::IconGlyph {
                glyph: *glyph,
                state: row.display_state(),
            },
        }
    }

    pub(crate) fn menu_icon(&self, config: &Config) -> Element<'a, GroupEvent> {
        self.glyph_icon()
            .view(config, 1.0)
            .unwrap_or_else(|| Space::new().into())
    }

    pub(crate) fn menu_value(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        match self {
            GroupSummary::Glyph { .. } => Row::new().into(),
            GroupSummary::Row { row, .. } => row.menu_value(config, clock),
        }
    }

    pub(crate) fn handle_view(
        &self,
        config: &'a Config,
        reveal_fraction: f32,
        clock: Clock,
    ) -> Element<'a, GroupEvent> {
        let hidden = 1.0 - reveal_fraction;
        let collapsing_value: Element<'a, GroupEvent> = match self {
            GroupSummary::Row { row, .. } if hidden > 0.0 => reveal(
                container(row.menu_value(config, clock))
                    .padding(Padding::ZERO.left(config.theme.spacing.item_gap)),
                hidden,
                Axis::Horizontal,
            )
            .into(),
            GroupSummary::Row { .. } | GroupSummary::Glyph { .. } => Row::new().into(),
        };
        Row::with_children([self.menu_icon(config), collapsing_value])
            .align_y(Alignment::Center)
            .into()
    }
}
