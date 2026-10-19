use sottomano_sources::ai_usage_spend::llama_server_slots::LlamaSlot;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{KeyValue, TooltipBody},
    widget_state::WidgetState,
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
        Phosphor::Stack(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Stack(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{}/{}", self.busy(), self.0.len()))
    }

    fn state(&self) -> WidgetState {
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

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("Processing", self.busy().to_string()),
            KeyValue::new("Slots", self.0.len().to_string()),
        ])
    }
}
