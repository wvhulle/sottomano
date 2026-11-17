use iced::{Color, Theme};

use super::pointer_tracked_row::RowPointerPresence;
use crate::theming::{
    background_strong_color, primary_weak_color, primary_weak_text_color, secondary_text_color,
    text_color,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRowEmphasis {
    Plain,
    Opened,
    Selected,
    Unavailable,
}

impl MenuRowEmphasis {
    #[must_use]
    pub fn text_color(self) -> fn(&Theme) -> Color {
        match self {
            MenuRowEmphasis::Plain | MenuRowEmphasis::Opened => text_color,
            MenuRowEmphasis::Selected => primary_weak_text_color,
            MenuRowEmphasis::Unavailable => secondary_text_color,
        }
    }

    #[must_use]
    pub fn secondary_glyph_color(self) -> fn(&Theme) -> Color {
        match self {
            MenuRowEmphasis::Plain | MenuRowEmphasis::Opened | MenuRowEmphasis::Unavailable => {
                secondary_text_color
            }
            MenuRowEmphasis::Selected => primary_weak_text_color,
        }
    }

    pub(super) fn background(
        self,
        theme: &Theme,
        pointer_presence: RowPointerPresence,
    ) -> Option<Color> {
        match (self, pointer_presence) {
            (MenuRowEmphasis::Selected, _) => Some(primary_weak_color(theme)),
            (MenuRowEmphasis::Opened, _)
            | (MenuRowEmphasis::Plain, RowPointerPresence::PointedAt) => {
                Some(background_strong_color(theme))
            }
            (MenuRowEmphasis::Plain, RowPointerPresence::NotPointedAt)
            | (MenuRowEmphasis::Unavailable, _) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::Theme;

    use super::{MenuRowEmphasis, RowPointerPresence};
    use crate::theming::{background_strong_color, primary_weak_color};

    #[test]
    fn a_selected_row_keeps_its_background_whether_or_not_the_pointer_is_over_it() {
        let theme = Theme::Light;
        let selected_color = Some(primary_weak_color(&theme));
        [
            RowPointerPresence::NotPointedAt,
            RowPointerPresence::PointedAt,
        ]
        .into_iter()
        .for_each(|pointer_presence| {
            assert_eq!(
                MenuRowEmphasis::Selected.background(&theme, pointer_presence),
                selected_color
            );
        });
    }

    #[test]
    fn an_opened_header_keeps_a_neutral_background_apart_from_the_selection() {
        let theme = Theme::Light;
        assert_eq!(
            MenuRowEmphasis::Opened.background(&theme, RowPointerPresence::NotPointedAt),
            Some(background_strong_color(&theme))
        );
        assert_ne!(
            Some(background_strong_color(&theme)),
            Some(primary_weak_color(&theme))
        );
    }

    #[test]
    fn a_plain_row_lights_up_only_under_the_pointer() {
        let theme = Theme::Light;
        assert_eq!(
            MenuRowEmphasis::Plain.background(&theme, RowPointerPresence::PointedAt),
            Some(background_strong_color(&theme))
        );
        assert_eq!(
            MenuRowEmphasis::Plain.background(&theme, RowPointerPresence::NotPointedAt),
            None
        );
    }
}
