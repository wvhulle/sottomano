use crate::{
    data::source::sysfs_sensors::x3d_vcache::X3dCachePreference,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        toggle_control::Toggle,
        tooltip_body::{KeyValue, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for X3dCachePreference {
    fn name(&self) -> &'static str {
        "X3D scheduling preference"
    }

    fn icon(&self) -> Glyph {
        match self {
            X3dCachePreference::Cache => Phosphor::Stack(self.state().weight()).into(),
            X3dCachePreference::Frequency => Phosphor::Lightning(self.state().weight()).into(),
        }
    }

    fn state(&self) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("Scheduler prefers", preferred_chiplet(*self)),
            KeyValue::new("Pressing prefers", preferred_chiplet(self.other())),
            KeyValue::new("Reverts", "at reboot, or by pressing again"),
        ])
    }
}

const fn preferred_chiplet(preference: X3dCachePreference) -> &'static str {
    match preference {
        X3dCachePreference::Cache => {
            "the stacked-cache chiplet, for cache-hungry work such as games"
        }
        X3dCachePreference::Frequency => {
            "the high-clock chiplet, for work that needs speed more than cache"
        }
    }
}

impl Toggle for X3dCachePreference {
    fn toggled(&self) -> Self {
        self.other()
    }

    fn toggle(&self) -> bool {
        self.other().write()
    }
}
