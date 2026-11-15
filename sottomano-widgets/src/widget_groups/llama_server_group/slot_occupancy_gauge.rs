use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::LlamaSlot;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
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
    pub fn reading_row(self) -> ReadingRow<'a> {
        let busy = self.busy();
        let fill = Ratio::new::<ratio>(busy as f64 / self.slots.len() as f64);
        let state = if busy > 0 {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Idle
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(format!("{}/{}", busy, self.slots.len())),
                gauge: ReadoutGauge::Gauged { fill, live: fill },
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                format!("{} busy slots", self.server),
                Phosphor::Stack(Weight::Regular).into(),
                state,
                Box::new(move |clock| self.hover_card(clock)),
            )
        }
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Occupancy",
            dictionary_table([
                LabeledValue::new("Processing", Quantity::count(self.busy())),
                LabeledValue::new("Slots", Quantity::count(self.slots.len())),
            ]),
        )]
    }
}
