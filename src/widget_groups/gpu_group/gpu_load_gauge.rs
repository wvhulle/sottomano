use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Frequency, Ratio},
        frequency::megahertz,
        ratio::percent,
    },
};

use crate::{
    data::source::gpu::{GpuModel, GpuSnapshot},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        hardware_section::HardwareEntries,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
    },
};

impl BarWidget for GpuSnapshot {
    fn icon(&self) -> Glyph {
        Phosphor::GraphicsCard(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::GraphicsCard(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.load)
    }

    fn tooltip(&self) -> TooltipBody {
        let percent_text =
            |share: Ratio| format!("{:.0}", share.into_format_args(percent, Abbreviation));
        let clock_text = |key: &'static str, clock: Option<Frequency>| {
            clock.map(|frequency| {
                KeyValue::new(
                    key,
                    format!("{:.0}", frequency.into_format_args(megahertz, Abbreviation)),
                )
            })
        };
        TooltipBody::sections(&[Section::untitled_dictionary(
            [
                Some(KeyValue::new("GPU", percent_text(self.load))),
                self.performance_state
                    .map(|state| KeyValue::new("Performance state", format!("P{state}"))),
                clock_text("Graphics clock", self.graphics_clock),
                clock_text("Memory clock", self.memory_clock),
                self.fan
                    .map(|speed| KeyValue::new("Fan", percent_text(speed))),
            ]
            .into_iter()
            .flatten(),
        )])
    }
}

impl HardwareEntries for GpuModel {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        vec![
            KeyValue::new("Model", self.name.as_str()),
            KeyValue::new("Driver", self.driver_version.as_str()),
        ]
    }
}
