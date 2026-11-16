use sottomano_desktop_sources::weather::open_meteo_pollen::{Allergen, AllergenConcentration};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
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

pub fn allergen_indicator(concentration: &AllergenConcentration) -> GroupIndicator<'_> {
    let weight = Weight::Regular;
    let glyph: IconGlyph = match concentration.allergen {
        Allergen::Alder => Phosphor::Leaf(weight),
        Allergen::Birch => Phosphor::Tree(weight),
        Allergen::Grass => Phosphor::Plant(weight),
        Allergen::Mugwort => Phosphor::Clover(weight),
        Allergen::Olive => Phosphor::TreeEvergreen(weight),
        Allergen::Ragweed => Phosphor::Flower(weight),
    }
    .into();
    let thresholds = ConcentrationThresholds::recommended_for(concentration.allergen);
    let state = if concentration.grains_per_cubic_metre >= thresholds.high {
        IndicatorState::Critical
    } else if concentration.grains_per_cubic_metre >= thresholds.moderate {
        IndicatorState::Warning
    } else {
        IndicatorState::Idle
    };
    let fill = allergen_gauge_fill(concentration);
    GroupIndicator::glyph_only(concentration.allergen.name(), glyph, state)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged { fill, live: fill },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| allergen_hover_card(concentration)))
}

fn allergen_gauge_fill(concentration: &AllergenConcentration) -> Ratio {
    let full_scale =
        ConcentrationThresholds::recommended_for(concentration.allergen).gauge_full_scale;
    let logarithmic_share = concentration.grains_per_cubic_metre.ln_1p() / full_scale.ln_1p();
    Ratio::new::<ratio>(logarithmic_share)
}

pub fn allergen_hover_card(concentration: &AllergenConcentration) -> Vec<Section> {
    let thresholds = ConcentrationThresholds::recommended_for(concentration.allergen);
    vec![
        Section::new(
            "Now",
            dictionary_table([LabeledValue::new(
                concentration.allergen.name(),
                Quantity::measured(
                    concentration.grains_per_cubic_metre,
                    DisplayUnit::GrainsPerCubicMeter,
                    1,
                ),
            )]),
        ),
        Section::new(
            "Thresholds",
            dictionary_table([
                LabeledValue::new("Moderate from", pollen_threshold(thresholds.moderate)),
                LabeledValue::new("High from", pollen_threshold(thresholds.high)),
            ]),
        ),
    ]
}

fn pollen_threshold(grains_per_cubic_metre: f64) -> Quantity {
    Quantity::measured(grains_per_cubic_metre, DisplayUnit::GrainsPerCubicMeter, 0)
}

pub(crate) const POLLEN_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Flower(Weight::Regular));

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
        allergen_gauge_fill(&grass(grains_per_cubic_metre)).get::<ratio>()
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
