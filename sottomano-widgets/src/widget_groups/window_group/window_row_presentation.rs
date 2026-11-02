use super::window_list::WindowListMessage;
use crate::widgets::{
    configured_theme::background_color, pointer_drag::slot_group::SlotGroup,
    window_title::WindowTitle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowSlotPresentation {
    Flat,
    InLocationGroup,
}

impl WindowSlotPresentation {
    pub(super) fn shows_icon(self) -> bool {
        self == WindowSlotPresentation::Flat
    }

    pub(super) fn sits_on_accent_tile(self, window_is_focused: bool) -> bool {
        self == WindowSlotPresentation::InLocationGroup && window_is_focused
    }

    pub(super) fn styled_title(
        self,
        window_title: WindowTitle<'_>,
        window_is_focused: bool,
    ) -> WindowTitle<'_> {
        match (self, self.sits_on_accent_tile(window_is_focused)) {
            (WindowSlotPresentation::Flat, _) => window_title,
            (WindowSlotPresentation::InLocationGroup, false) => window_title.without_location(),
            (WindowSlotPresentation::InLocationGroup, true) => {
                window_title.without_location().color(background_color)
            }
        }
    }
}

pub(super) struct ArrangedWindowRows<'a> {
    pub(super) groups: Vec<SlotGroup<'a, WindowListMessage>>,
    pub(super) presentations: Vec<WindowSlotPresentation>,
}
