use crate::{
    data::source::clight::{self, Clight},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor,
        toggle_control::Toggle,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Clight {
    fn icon(&self) -> Glyph {
        if self.auto {
            phosphor::Eye::Regular.into()
        } else {
            phosphor::EyeSlash::Regular.into()
        }
    }

    fn disabled(&self) -> bool {
        !self.auto
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![row(
            "Auto-brightness",
            if self.auto { "on" } else { "off — manual" },
        )])
    }
}

impl Toggle for Clight {
    fn toggled(&self) -> Self {
        Clight { auto: !self.auto }
    }

    fn toggle(&self) -> bool {
        clight::flip()
    }
}
