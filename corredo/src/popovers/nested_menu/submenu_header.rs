use iced::{
    Color, Element, Theme,
    widget::{text, text::Shaping},
};

use super::MenuBuilder;
use crate::{
    popovers::{
        CascadeDirection, MenuEntry, MenuRow, MenuRowContent, MenuRowEmphasis, NestedMenuAction,
        NestedMenuLayout,
        menu_row::{RowHoverReaction, SatelliteReach},
    },
    transitions::ExpansionState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretPointing {
    Rightward,
    Leftward,
    Downward,
    Nowhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SubmenuContents {
    Entries,
    Nothing,
}

impl<SubmenuKey, Message> From<&[MenuEntry<'_, SubmenuKey, Message>]> for SubmenuContents {
    fn from(entries: &[MenuEntry<'_, SubmenuKey, Message>]) -> Self {
        if entries.is_empty() {
            SubmenuContents::Nothing
        } else {
            SubmenuContents::Entries
        }
    }
}

#[must_use]
pub fn submenu_header_emphasis(
    layout: NestedMenuLayout,
    expansion_state: ExpansionState,
) -> MenuRowEmphasis {
    match (layout, expansion_state) {
        (NestedMenuLayout::Cascading, ExpansionState::Expanded) => MenuRowEmphasis::Selected,
        (NestedMenuLayout::Accordion, ExpansionState::Expanded) => MenuRowEmphasis::Opened,
        (_, ExpansionState::Collapsed) => MenuRowEmphasis::Plain,
    }
}

pub(super) fn text_caret<'view, Message: 'view>(
    pointing: CaretPointing,
    color: fn(&Theme) -> Color,
) -> Element<'view, Message> {
    let glyph = match pointing {
        CaretPointing::Rightward => "\u{25b8}",
        CaretPointing::Leftward => "\u{25c2}",
        CaretPointing::Downward => "\u{25be}",
        CaretPointing::Nowhere => "\u{2205}",
    };
    text(glyph)
        .shaping(Shaping::Advanced)
        .style(move |theme| text::Style {
            color: Some(color(theme)),
        })
        .into()
}

impl<'view, SubmenuKey: Clone + PartialEq + 'view, Message: Clone + 'view>
    MenuBuilder<'view, SubmenuKey, Message>
{
    pub(super) fn header_row(
        &self,
        header: MenuRowContent<'view, Message>,
        path: &[SubmenuKey],
        expansion_state: ExpansionState,
        satellite_reach: SatelliteReach,
        submenu_contents: SubmenuContents,
    ) -> MenuRow<'view, Message> {
        let layout = self.presentation.layout;
        let emphasis = match submenu_contents {
            SubmenuContents::Entries => submenu_header_emphasis(layout, expansion_state),
            SubmenuContents::Nothing => MenuRowEmphasis::Unavailable,
        };
        let caret_color = emphasis.secondary_glyph_color();
        let caret_pointing = |pointing| match submenu_contents {
            SubmenuContents::Entries => pointing,
            SubmenuContents::Nothing => CaretPointing::Nowhere,
        };
        let mut header = header;
        match (layout, self.chrome.orientation.direction, expansion_state) {
            (NestedMenuLayout::Cascading, CascadeDirection::Leftward, _) => header.leading.insert(
                0,
                (self.caret)(caret_pointing(CaretPointing::Leftward), caret_color),
            ),
            (NestedMenuLayout::Cascading, CascadeDirection::Rightward, _)
            | (NestedMenuLayout::Accordion, _, ExpansionState::Collapsed) => header.trailing.push(
                (self.caret)(caret_pointing(CaretPointing::Rightward), caret_color),
            ),
            (NestedMenuLayout::Accordion, _, ExpansionState::Expanded) => header.trailing.push(
                (self.caret)(caret_pointing(CaretPointing::Downward), caret_color),
            ),
        }
        let row = MenuRow::new(header).emphasis(emphasis);
        match (submenu_contents, layout) {
            (SubmenuContents::Nothing, _) => row,
            (SubmenuContents::Entries, NestedMenuLayout::Cascading) => {
                let reveal = (self.on_action)(NestedMenuAction::Opened(path.to_vec()));
                row.on_press(reveal.clone())
                    .hover_reaction(RowHoverReaction::Reveals {
                        reveal,
                        satellite_reach,
                        reveal_delay: self.chrome.style.reveal_delay,
                    })
            }
            (SubmenuContents::Entries, NestedMenuLayout::Accordion) => row.on_press((self
                .on_action)(
                self.state
                    .toggling(path, self.presentation.open_group_limit()),
            )),
        }
    }
}
