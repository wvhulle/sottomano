use sottomano_sources::weather::open_meteo_pollen::{Allergen, AllergenConcentration, Pollen};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

struct ConcentrationThresholds {
    moderate: f64,
    high: f64,
    gauge_full_scale: f64,
}

impl ConcentrationThresholds {
    const fn recommended_for(allergen: Allergen) -> Self {
        match allergen {
            Allergen::Alder => ConcentrationThresholds {
                moderate: 10.0,
                high: 50.0,
                gauge_full_scale: 100.0,
            },
            Allergen::Birch => ConcentrationThresholds {
                moderate: 10.0,
                high: 45.0,
                gauge_full_scale: 100.0,
            },
            Allergen::Grass => ConcentrationThresholds {
                moderate: 20.0,
                high: 50.0,
                gauge_full_scale: 80.0,
            },
            Allergen::Mugwort => ConcentrationThresholds {
                moderate: 10.0,
                high: 30.0,
                gauge_full_scale: 50.0,
            },
            Allergen::Olive => ConcentrationThresholds {
                moderate: 50.0,
                high: 200.0,
                gauge_full_scale: 400.0,
            },
            Allergen::Ragweed => ConcentrationThresholds {
                moderate: 5.0,
                high: 20.0,
                gauge_full_scale: 50.0,
            },
        }
    }
}

impl GroupItemReading for AllergenConcentration {
    fn name(&self) -> &'static str {
        "Allergen"
    }

    fn icon(&self) -> IconGlyph {
        let weight = Weight::Regular;
        match self.allergen {
            Allergen::Alder => Phosphor::Leaf(weight),
            Allergen::Birch => Phosphor::Tree(weight),
            Allergen::Grass => Phosphor::Plant(weight),
            Allergen::Mugwort => Phosphor::Clover(weight),
            Allergen::Olive => Phosphor::TreeEvergreen(weight),
            Allergen::Ragweed => Phosphor::Flower(weight),
        }
        .into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        let thresholds = ConcentrationThresholds::recommended_for(self.allergen);
        if self.grains_per_cubic_metre >= thresholds.high {
            ReadingDisplayState::Critical
        } else if self.grains_per_cubic_metre >= thresholds.moderate {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        let full_scale = ConcentrationThresholds::recommended_for(self.allergen).gauge_full_scale;
        let logarithmic_share = self.grains_per_cubic_metre.ln_1p() / full_scale.ln_1p();
        Some(Ratio::new::<ratio>(logarithmic_share))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let thresholds = ConcentrationThresholds::recommended_for(self.allergen);
        HoverCardBody::sections([
            Section::dictionary([LabeledValue::new(
                self.allergen.name(),
                Quantity::measured(
                    self.grains_per_cubic_metre,
                    DisplayUnit::GrainsPerCubicMeter,
                    1,
                ),
            )]),
            Section::dictionary([
                LabeledValue::new("Moderate from", pollen_threshold(thresholds.moderate)),
                LabeledValue::new("High from", pollen_threshold(thresholds.high)),
            ])
            .titled("Thresholds"),
        ])
    }
}

fn pollen_threshold(grains_per_cubic_metre: f64) -> Quantity {
    Quantity::measured(grains_per_cubic_metre, DisplayUnit::GrainsPerCubicMeter, 0)
}

impl GroupItemReading for Pollen {
    fn name(&self) -> &'static str {
        "Pollen"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Flower(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Flower(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grass(grains_per_cubic_metre: f64) -> AllergenConcentration {
        AllergenConcentration {
            allergen: Allergen::Grass,
            grains_per_cubic_metre,
        }
    }

    fn gauge_fill(grains_per_cubic_metre: f64) -> f64 {
        grass(grains_per_cubic_metre)
            .gauge()
            .expect("allergens always have a gauge")
            .get::<ratio>()
    }

    #[test]
    fn gauge_is_empty_without_pollen_and_full_at_full_scale() {
        assert!(gauge_fill(0.0).abs() < f64::EPSILON);
        let full_scale = ConcentrationThresholds::recommended_for(Allergen::Grass).gauge_full_scale;
        assert!((gauge_fill(full_scale) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trace_amounts_below_one_grain_still_show() {
        let trace = gauge_fill(0.7);
        assert!(trace > 0.05 && trace < gauge_fill(2.0));
    }
}
