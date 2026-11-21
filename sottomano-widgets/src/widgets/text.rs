use corredo::{
    changing_text::{IntroEffect, PlaceholderEffect, ReplaceEffect, TextEffects},
    theming::{FontFaces, LINE_HEIGHT, TextFace},
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

pub fn column_heading<'view>(
    font: &ResolvedFont,
    title: impl IntoFragment<'view>,
) -> Text<'view, iced::Theme, iced::Renderer> {
    panel_text(font, title)
        .size(font.small_text_size_px)
        .style(text::secondary)
}

pub fn row_key_label<'view>(
    font: &ResolvedFont,
    title: impl IntoFragment<'view>,
) -> Text<'view, iced::Theme, iced::Renderer> {
    panel_text(font, title).style(text::secondary)
}

pub fn panel_text<'view>(
    font: &ResolvedFont,
    content: impl IntoFragment<'view>,
) -> Text<'view, iced::Theme, iced::Renderer> {
    text(content)
        .size(font.text_size_px)
        .font(resolved_font_faces(font).regular)
        .line_height(LINE_HEIGHT)
        .shaping(Shaping::Advanced)
}

#[must_use]
pub fn resolved_font_faces(font: &ResolvedFont) -> FontFaces {
    FontFaces::from(match font.role {
        FontRole::Text => Family::SansSerif,
        FontRole::Monospace => Family::Monospace,
    })
}

#[must_use]
pub fn resolved_text_face(font: &ResolvedFont) -> TextFace {
    TextFace {
        font: resolved_font_faces(font).regular,
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
pub fn hover_card_text_effects(loading: configured_effects::HoverCardLoading) -> TextEffects {
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
    use corredo::changing_text::TextEffects;
    use sottomano_toml_config::hover_cards::HoverCardLoading;

    use super::hover_card_text_effects;

    #[test]
    fn default_configured_effects_match_the_widget_defaults() {
        assert_eq!(
            hover_card_text_effects(HoverCardLoading::default()),
            TextEffects::default()
        );
    }
}
