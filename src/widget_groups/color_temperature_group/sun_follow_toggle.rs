use crate::{
    data::source::screen_temperature::{self, SunFollow},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        toggle_control::Toggle,
        tooltip_body::{TooltipBody, row},
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
    },
};

impl BarWidget for SunFollow {
    fn icon(&self) -> Glyph {
        if self.auto {
            WeatherIcon::DaySunny.into()
        } else {
            Phosphor::Prohibit(self.state().weight()).into()
        }
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.auto)
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
