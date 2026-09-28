pub use phosphor_icons::{Phosphor, Weight};

use crate::widgets::bar_widget::Glyph;

impl From<Phosphor> for Glyph {
    fn from(icon: Phosphor) -> Self {
        Glyph::Phosphor(icon)
    }
}
