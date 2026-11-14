use corredo::{
    drag_reorder::{SlotGroup, UngroupedHighlightLook},
    theming::background_color,
};

use super::window_list::WindowListInteraction;
use crate::widgets::window_title::WindowTitle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowTileMarking {
    Marked,
    Unmarked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowSlotPresentation {
    Flat,
    InKeyedGroup,
    OutsideKeyedGroups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotBackdrop {
    OpaqueTile,
    Bare,
}

impl WindowSlotPresentation {
    pub(super) fn slot_backdrop(self, tile_marking: WindowTileMarking) -> SlotBackdrop {
        match (self, tile_marking) {
            (WindowSlotPresentation::Flat, _) | (_, WindowTileMarking::Unmarked) => {
                SlotBackdrop::Bare
            }
            (
                WindowSlotPresentation::InKeyedGroup | WindowSlotPresentation::OutsideKeyedGroups,
                WindowTileMarking::Marked,
            ) => SlotBackdrop::OpaqueTile,
        }
    }

    pub(super) fn styled_title(
        self,
        window_title: WindowTitle<'_>,
        tile_marking: WindowTileMarking,
    ) -> WindowTitle<'_> {
        match (self, self.slot_backdrop(tile_marking)) {
            (WindowSlotPresentation::Flat, _)
            | (WindowSlotPresentation::OutsideKeyedGroups, SlotBackdrop::Bare) => window_title,
            (WindowSlotPresentation::OutsideKeyedGroups, SlotBackdrop::OpaqueTile) => {
                window_title.color(background_color)
            }
            (WindowSlotPresentation::InKeyedGroup, SlotBackdrop::Bare) => {
                window_title.without_group_key()
            }
            (WindowSlotPresentation::InKeyedGroup, SlotBackdrop::OpaqueTile) => {
                window_title.without_group_key().color(background_color)
            }
        }
    }
}

pub(super) struct ArrangedWindowRows<'a> {
    pub(super) groups: Vec<SlotGroup<'a, WindowListInteraction>>,
    pub(super) presentations: Vec<WindowSlotPresentation>,
    pub(super) ungrouped_highlight_look: UngroupedHighlightLook,
}

#[cfg(test)]
mod tests {
    use super::{SlotBackdrop, WindowSlotPresentation, WindowTileMarking};

    #[test]
    fn window_outside_groups_sits_on_the_tile_only_while_focused() {
        let presentation = WindowSlotPresentation::OutsideKeyedGroups;
        assert_eq!(
            presentation.slot_backdrop(WindowTileMarking::Marked),
            SlotBackdrop::OpaqueTile
        );
        assert_eq!(
            presentation.slot_backdrop(WindowTileMarking::Unmarked),
            SlotBackdrop::Bare
        );
    }

    #[test]
    fn flat_window_never_sits_on_the_tile() {
        assert_eq!(
            WindowSlotPresentation::Flat.slot_backdrop(WindowTileMarking::Marked),
            SlotBackdrop::Bare
        );
    }
}
