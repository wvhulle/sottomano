use crate::{
    data::source::umbra::{self, Umbra},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::Toggle,
        tooltip_body::{Section, TooltipBody},
    },
};

impl BarWidget for Umbra {
    fn icon(&self) -> Glyph {
        if self.dark {
            Phosphor::Moon(Weight::Regular).into()
        } else {
            Phosphor::Sun(Weight::Regular).into()
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled(&[(
            "Mode",
            if self.dark { "Dark" } else { "Light" },
        )])])
    }
}

impl Toggle for Umbra {
    fn toggled(&self) -> Self {
        Umbra { dark: !self.dark }
    }

    fn toggle(&self) -> bool {
        umbra::toggle_mode()
    }
}
