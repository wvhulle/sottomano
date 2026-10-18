use sottomano_sources::{
    bus,
    clight::{self, Clight},
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::Phosphor,
    toggle_control::ToggleableSource,
    tooltip_body::TooltipBody,
    widget_state::WidgetState,
};

impl BarWidget for Clight {
    fn name(&self) -> &'static str {
        "Auto-brightness"
    }

    fn icon(&self) -> Glyph {
        if self.auto {
            Phosphor::Eye(self.state().weight()).into()
        } else {
            Phosphor::EyeSlash(self.state().weight()).into()
        }
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.auto)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([])
    }
}

impl ToggleableSource for Clight {
    fn toggled_value(&self) -> Self {
        Clight { auto: !self.auto }
    }

    fn apply_toggle(&self) -> Result<(), bus::Error> {
        clight::flip()
    }

    fn press_action_text(&self) -> &'static str {
        if self.auto {
            "stop setting brightness from webcam light; keep the current level"
        } else {
            "set brightness from the light the webcam sees"
        }
    }
}
