use iced::{Font, widget::text::Span};

use crate::{
    config::{Config, theme::font::ResolvedFont},
    data::source::app_rules::TitleParts,
    widgets::text::{FontFallbacks, colored_span},
};

const SEPARATOR: &str = " · ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleRole {
    Lead,
    Main,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleRun {
    pub text: String,
    pub font: Font,
    pub role: TitleRole,
}

pub fn title_runs(
    font: &ResolvedFont,
    fallbacks: &FontFallbacks,
    parts: Option<&TitleParts>,
    lead_first: bool,
) -> Vec<TitleRun> {
    let Some(parts) = parts else {
        return Vec::new();
    };
    let runs = |content: &str, base: Font, role: TitleRole| {
        fallbacks
            .runs(font, content, base)
            .into_iter()
            .map(move |(text, font)| TitleRun { text, font, role })
            .collect::<Vec<_>>()
    };
    let main = runs(&parts.main, font.iced_font, TitleRole::Main);
    let Some(lead) = &parts.lead else {
        return main;
    };
    let lead = runs(lead, font.italic(), TitleRole::Lead);
    let separator = runs(SEPARATOR, font.iced_font, TitleRole::Lead);
    if lead_first {
        lead.into_iter().chain(separator).chain(main).collect()
    } else {
        main.into_iter().chain(separator).chain(lead).collect()
    }
}

pub fn title_spans<'a>(config: &Config, runs: &[TitleRun], opacity: f32) -> Vec<Span<'a>> {
    runs.iter()
        .map(|run| {
            let color = match run.role {
                TitleRole::Lead => config.theme.palette.inactive,
                TitleRole::Main => config.theme.palette.bar_foreground,
            };
            colored_span(run.text.clone(), run.font, color.scale_alpha(opacity))
        })
        .collect()
}
