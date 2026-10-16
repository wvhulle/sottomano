use crate::{
    data::source::screen_temperature::{self, SunFollow},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        toggle_control::Toggle,
        tooltip_body::TooltipBody,
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
    },
};

impl BarWidget for SunFollow {
    fn name(&self) -> &'static str {
        "Sun follow"
    }

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
        TooltipBody::sections([])
    }
}

impl Toggle for SunFollow {
    fn toggled(&self) -> Self {
        SunFollow {
            auto: !self.auto,
            unit: self.unit.clone(),
        }
    }

    fn press_does(&self) -> &'static str {
        if self.auto {
            "stop shifting screen colour with the sun; keep the current temperature"
        } else {
            "warm the screen at sunset and cool it at sunrise again"
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
