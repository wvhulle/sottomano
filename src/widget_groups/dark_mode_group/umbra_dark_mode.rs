use crate::{
    data::source::umbra::{self, Umbra},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        toggle_control::Toggle,
        tooltip_body::{TooltipBody, row},
        weather_icons::WeatherIcon,
    },
};

impl BarWidget for Umbra {
    fn icon(&self) -> Glyph {
        if self.dark {
            WeatherIcon::NightClear.into()
        } else {
            WeatherIcon::DaySunny.into()
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![row("Mode", if self.dark { "Dark" } else { "Light" })])
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
