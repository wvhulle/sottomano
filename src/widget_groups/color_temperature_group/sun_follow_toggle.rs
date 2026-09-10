use crate::{
    data::source::screen_temperature::{self, SunFollow},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        toggle_control::Toggle,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for SunFollow {
    fn icon(&self) -> Glyph {
        if self.auto {
            Solid::Sun.into()
        } else {
            Solid::Ban.into()
        }
    }

    fn disabled(&self) -> bool {
        !self.auto
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![row(
            "Auto-temperature",
            if self.auto { "on" } else { "off — manual" },
        )])
    }
}

impl Toggle for SunFollow {
    fn toggled(&self) -> Self {
        SunFollow {
            auto: !self.auto,
            unit: self.unit.clone(),
        }
    }

    fn toggle(&self) -> bool {
        if self.auto {
            screen_temperature::stop(&self.unit)
        } else {
            screen_temperature::restart(&self.unit)
        }
    }
}
