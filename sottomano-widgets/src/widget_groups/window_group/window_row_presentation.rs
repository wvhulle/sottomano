use super::window_list::WindowListMessage;
use crate::widgets::{
    configured_theme::background_color,
    pointer_drag::{slot_group::SlotGroup, slot_pager::UngroupedHighlightLook},
    window_title::WindowTitle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowSlotPresentation {
    Flat,
    InKeyedGroup,
    OutsideKeyedGroups,
}

impl WindowSlotPresentation {
    pub(super) fn shows_icon(self) -> bool {
        self != WindowSlotPresentation::InKeyedGroup
    }

    pub(super) fn marks_urgency_with_tile(self) -> bool {
        self != WindowSlotPresentation::Flat
    }

    pub(super) fn sits_on_opaque_tile(self, window_is_marked: bool) -> bool {
        self != WindowSlotPresentation::Flat && window_is_marked
    }

    pub(super) fn styled_title(
        self,
        window_title: WindowTitle<'_>,
        window_is_marked: bool,
    ) -> WindowTitle<'_> {
        match (self, self.sits_on_opaque_tile(window_is_marked)) {
            (WindowSlotPresentation::Flat, _)
            | (WindowSlotPresentation::OutsideKeyedGroups, false) => window_title,
            (WindowSlotPresentation::OutsideKeyedGroups, true) => {
                window_title.color(background_color)
            }
            (WindowSlotPresentation::InKeyedGroup, false) => window_title.without_group_key(),
            (WindowSlotPresentation::InKeyedGroup, true) => {
                window_title.without_group_key().color(background_color)
            }
        }
    }
}

pub(super) struct ArrangedWindowRows<'a> {
    pub(super) groups: Vec<SlotGroup<'a, WindowListMessage>>,
    pub(super) presentations: Vec<WindowSlotPresentation>,
    pub(super) ungrouped_highlight_look: UngroupedHighlightLook,
}

#[cfg(test)]
mod tests {
    use super::WindowSlotPresentation;

    #[test]
    fn window_outside_groups_sits_on_the_tile_only_while_focused() {
        let presentation = WindowSlotPresentation::OutsideKeyedGroups;
        assert!(presentation.sits_on_opaque_tile(true));
        assert!(!presentation.sits_on_opaque_tile(false));
    }

    #[test]
    fn window_outside_groups_keeps_its_own_icon() {
        assert!(WindowSlotPresentation::OutsideKeyedGroups.shows_icon());
    }

    #[test]
    fn flat_window_never_sits_on_the_tile() {
        assert!(!WindowSlotPresentation::Flat.sits_on_opaque_tile(true));
    }

    #[test]
    fn only_flat_windows_mark_urgency_with_a_line() {
        assert!(!WindowSlotPresentation::Flat.marks_urgency_with_tile());
        assert!(WindowSlotPresentation::InKeyedGroup.marks_urgency_with_tile());
        assert!(WindowSlotPresentation::OutsideKeyedGroups.marks_urgency_with_tile());
    }
}
