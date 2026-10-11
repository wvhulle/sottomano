pub mod font;
pub mod palette;

use serde::{Deserialize, Serialize};

use crate::config::theme::{
    font::{FontChoice, ResolvedFont},
    palette::Palette,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeChoice {
    pub palette: Palette,
    pub font: FontChoice,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: Palette,
    pub font: ResolvedFont,
    pub monospace_font: ResolvedFont,
}
