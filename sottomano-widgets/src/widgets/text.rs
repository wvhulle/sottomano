pub use corredo::font_faces::LINE_HEIGHT;
use corredo::{
    font_faces::{FontFaces, TextFace},
    text_effects::{IntroEffect, PlaceholderEffect, ReplaceEffect, TextEffects},
};
use iced::{
    advanced::graphics::text::font_system,
    font::Family,
    widget::{
        Text, text,
        text::{IntoFragment, Shaping},
    },
};
use sottomano_toml_config::{
    hover_cards as configured_effects,
    theme::{
        Theme,
        font::{FontRole, ResolvedFont},
    },
};

pub fn column_heading<'a>(
    font: &ResolvedFont,
    title: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    panel_text(font, title)
        .size(font.small_text_size_px)
        .style(text::secondary)
}

pub fn row_key_label<'a>(
    font: &ResolvedFont,
    title: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    panel_text(font, title).font(font_faces_of(font).italic)
}

pub fn panel_text<'a>(
    font: &ResolvedFont,
    content: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    text(content)
        .size(font.text_size_px)
        .font(font_faces_of(font).regular)
        .line_height(LINE_HEIGHT)
        .shaping(Shaping::Advanced)
}

#[must_use]
pub fn font_faces_of(font: &ResolvedFont) -> FontFaces {
    FontFaces::of_family(match font.role {
        FontRole::Text => Family::SansSerif,
        FontRole::Monospace => Family::Monospace,
    })
}

#[must_use]
pub fn text_face_of(font: &ResolvedFont) -> TextFace {
    TextFace {
        font: font_faces_of(font).regular,
        size_px: font.text_size_px,
    }
}

pub fn point_generic_font_families_at(theme: &Theme) {
    let mut fonts = font_system().write().expect("font system lock poisoned");
    let database = fonts.raw().db_mut();
    database.set_sans_serif_family(theme.font.family.clone());
    database.set_monospace_family(theme.monospace_font.family.clone());
}

#[must_use]
pub fn text_effects_of(loading: configured_effects::HoverCardLoading) -> TextEffects {
    TextEffects {
        placeholder: match loading.placeholder {
            configured_effects::PlaceholderEffect::Pulse { period } => {
                PlaceholderEffect::Pulse { period }
            }
            configured_effects::PlaceholderEffect::Shimmer { sweep_period } => {
                PlaceholderEffect::Shimmer { sweep_period }
            }
            configured_effects::PlaceholderEffect::Noise { glyph_change } => {
                PlaceholderEffect::Noise { glyph_change }
            }
        },
        intro: match loading.intro {
            configured_effects::IntroEffect::Fade { duration } => IntroEffect::Fade { duration },
            configured_effects::IntroEffect::Decode {
                duration,
                glyph_change,
            } => IntroEffect::Decode {
                duration,
                glyph_change,
            },
            configured_effects::IntroEffect::Wipe { duration } => IntroEffect::Wipe { duration },
            configured_effects::IntroEffect::Typewriter { per_char } => {
                IntroEffect::Typewriter { per_char }
            }
            configured_effects::IntroEffect::Immediate => IntroEffect::Immediate,
        },
        replace: match loading.replace {
            configured_effects::ReplaceEffect::Decode {
                duration,
                glyph_change,
            } => ReplaceEffect::Decode {
                duration,
                glyph_change,
            },
            configured_effects::ReplaceEffect::Crossfade { duration } => {
                ReplaceEffect::Crossfade { duration }
            }
            configured_effects::ReplaceEffect::Roll { duration } => {
                ReplaceEffect::Roll { duration }
            }
            configured_effects::ReplaceEffect::Flash { duration } => {
                ReplaceEffect::Flash { duration }
            }
            configured_effects::ReplaceEffect::Immediate => ReplaceEffect::Immediate,
        },
    }
}

#[cfg(test)]
mod tests {
    use corredo::text_effects::TextEffects;
    use sottomano_toml_config::hover_cards::HoverCardLoading;

    use super::text_effects_of;

    #[test]
    fn default_configured_effects_match_the_widget_defaults() {
        assert_eq!(
            text_effects_of(HoverCardLoading::default()),
            TextEffects::default()
        );
    }
}
