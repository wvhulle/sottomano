pub mod font;
pub mod palette;
pub mod spacing_tokens;

use serde::{Deserialize, Serialize};

use crate::{
    spacing::Spacing,
    theme::{
        font::{FontChoice, ResolvedFont},
        palette::Palette,
        spacing_tokens::SpacingTokens,
    },
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeChoice {
    pub palette: Palette,
    pub font: FontChoice,
    pub spacing: SpacingTokens<Spacing>,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: Palette,
    pub font: ResolvedFont,
    pub monospace_font: ResolvedFont,
    pub spacing: SpacingTokens,
}
