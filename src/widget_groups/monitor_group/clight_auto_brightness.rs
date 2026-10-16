use crate::{
    data::source::clight::{self, Clight},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        toggle_control::Toggle,
        tooltip_body::TooltipBody,
        widget_state::WidgetState,
    },
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

impl Toggle for Clight {
    fn toggled(&self) -> Self {
        Clight { auto: !self.auto }
    }

    fn toggle(&self) -> bool {
        clight::flip()
    }

    fn press_does(&self) -> &'static str {
        if self.auto {
            "stop setting brightness from webcam light; keep the current level"
        } else {
            "set brightness from the light the webcam sees"
        }
    }
}
