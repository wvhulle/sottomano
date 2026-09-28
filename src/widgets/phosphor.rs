#![allow(clippy::absolute_paths)]

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

impl Phosphor {
    pub fn from_brand_or_slug(handle: &str) -> ::core::option::Option<Phosphor> {
        Phosphor::from_slug(&format!("{handle}-logo")).or_else(|| Phosphor::from_slug(handle))
    }
}

impl ::core::convert::From<Phosphor> for crate::widgets::bar_widget::Glyph {
    fn from(icon: Phosphor) -> Self {
        crate::widgets::bar_widget::Glyph::Phosphor(icon)
    }
}

impl ::core::convert::From<Phosphor> for crate::widgets::tooltip_body::Cell {
    fn from(icon: Phosphor) -> Self {
        crate::widgets::tooltip_body::Cell::Glyph(icon.into())
    }
}
