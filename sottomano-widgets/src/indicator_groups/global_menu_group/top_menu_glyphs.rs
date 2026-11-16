use crate::widgets::phosphor::{Phosphor, Weight};

const TOP_MENU_KEYWORD_GLYPHS: [TopMenuKeywordGlyph; 14] = [
    TopMenuKeywordGlyph::new(&["file"], Phosphor::Folder(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["edit"], Phosphor::PencilSimple(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["view"], Phosphor::Eye(Weight::Regular)),
    TopMenuKeywordGlyph::new(
        &["settings", "preferences", "options"],
        Phosphor::Gear(Weight::Regular),
    ),
    TopMenuKeywordGlyph::new(&["help"], Phosphor::Question(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["tools"], Phosphor::Wrench(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["window"], Phosphor::AppWindow(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["go", "navigate"], Phosphor::Compass(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["bookmarks"], Phosphor::BookmarkSimple(Weight::Regular)),
    TopMenuKeywordGlyph::new(
        &["history"],
        Phosphor::ClockCounterClockwise(Weight::Regular),
    ),
    TopMenuKeywordGlyph::new(&["insert"], Phosphor::PlusSquare(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["format"], Phosphor::TextAa(Weight::Regular)),
    TopMenuKeywordGlyph::new(&["image"], Phosphor::Image(Weight::Regular)),
    TopMenuKeywordGlyph::new(
        &["select", "selection"],
        Phosphor::Selection(Weight::Regular),
    ),
];

struct TopMenuKeywordGlyph {
    keywords: &'static [&'static str],
    glyph: Phosphor,
}

impl TopMenuKeywordGlyph {
    const fn new(keywords: &'static [&'static str], glyph: Phosphor) -> Self {
        Self { keywords, glyph }
    }
}

#[must_use]
pub fn top_menu_glyph(top_menu_label: &str) -> Phosphor {
    let label_words: Vec<String> = top_menu_label
        .split(|label_char: char| !label_char.is_alphanumeric())
        .filter(|label_word| !label_word.is_empty())
        .map(str::to_lowercase)
        .collect();
    TOP_MENU_KEYWORD_GLYPHS
        .iter()
        .find(|keyword_glyph| {
            keyword_glyph
                .keywords
                .iter()
                .any(|keyword| label_words.iter().any(|label_word| label_word == keyword))
        })
        .map_or(Phosphor::ListDashes(Weight::Regular), |keyword_glyph| {
            keyword_glyph.glyph
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_menu_names_get_their_glyph() {
        assert_eq!(top_menu_glyph("File"), Phosphor::Folder(Weight::Regular));
        assert_eq!(top_menu_glyph("Settings"), Phosphor::Gear(Weight::Regular));
        assert_eq!(top_menu_glyph("Help"), Phosphor::Question(Weight::Regular));
    }

    #[test]
    fn matching_ignores_case_and_whole_words_only() {
        assert_eq!(
            top_menu_glyph("EDIT"),
            Phosphor::PencilSimple(Weight::Regular)
        );
        assert_eq!(top_menu_glyph("Go to"), Phosphor::Compass(Weight::Regular));
        assert_eq!(
            top_menu_glyph("Google"),
            Phosphor::ListDashes(Weight::Regular)
        );
    }

    #[test]
    fn unknown_or_translated_names_fall_back() {
        assert_eq!(
            top_menu_glyph("Scan"),
            Phosphor::ListDashes(Weight::Regular)
        );
        assert_eq!(
            top_menu_glyph("Bestand"),
            Phosphor::ListDashes(Weight::Regular)
        );
    }
}
