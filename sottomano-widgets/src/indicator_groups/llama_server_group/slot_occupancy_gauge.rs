use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::LlamaSlot;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct SlotOccupancy<'a> {
    pub slots: &'a [LlamaSlot],
    pub server: &'a str,
}

impl<'a> SlotOccupancy<'a> {
    fn busy(&self) -> usize {
        self.slots.iter().filter(|slot| slot.processing).count()
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn indicator(self) -> GroupIndicator<'a> {
        let busy = self.busy();
        let fill = Ratio::new::<ratio>(busy as f64 / self.slots.len() as f64);
        let state = if busy > 0 {
            IndicatorState::On
        } else {
            IndicatorState::Idle
        };
        GroupIndicator::glyph_only(
            format!("{} busy slots", self.server),
            Phosphor::Stack(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(format!("{}/{}", busy, self.slots.len())),
            gauge: ReadoutGauge::Gauged { fill, live: fill },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Occupancy",
            dictionary_table([
                LabeledValue::new("Processing", Quantity::count(self.busy())),
                LabeledValue::new("Slots", Quantity::count(self.slots.len())),
            ]),
        )]
    }
}
