use chrono::Locale;
use sottomano_sources::ai_usage_spend::llama_server_slots::LlamaSlot;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

pub struct SlotOccupancy<'a>(pub &'a [LlamaSlot]);

impl SlotOccupancy<'_> {
    fn busy(&self) -> usize {
        self.0.iter().filter(|slot| slot.processing).count()
    }
}

impl BarWidget for SlotOccupancy<'_> {
    fn name(&self) -> &'static str {
        "Busy slots"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Stack(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Stack(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{}/{}", self.busy(), self.0.len()))
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.busy() > 0 {
            WidgetState::On
        } else {
            WidgetState::Idle
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<ratio>(
            self.busy() as f64 / self.0.len() as f64,
        ))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Processing", Quantity::count(self.busy())),
            LabeledValue::new("Slots", Quantity::count(self.0.len())),
        ])
    }
}
