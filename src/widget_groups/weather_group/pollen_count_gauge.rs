use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    data::source::weather::{Allergen, AllergenConcentration, Pollen},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

struct ConcentrationThresholds {
    moderate: f64,
    high: f64,
    gauge_full_scale: f64,
}

impl AllergenConcentration {
    const fn thresholds(&self) -> ConcentrationThresholds {
        match self.allergen {
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

    const fn species_icon(&self, weight: Weight) -> Phosphor {
        match self.allergen {
            Allergen::Alder => Phosphor::Leaf(weight),
            Allergen::Birch => Phosphor::Tree(weight),
            Allergen::Grass => Phosphor::Plant(weight),
            Allergen::Mugwort => Phosphor::Clover(weight),
            Allergen::Olive => Phosphor::TreeEvergreen(weight),
            Allergen::Ragweed => Phosphor::Flower(weight),
        }
    }
}

impl BarWidget for AllergenConcentration {
    fn name(&self) -> &'static str {
        "Allergen"
    }

    fn icon(&self) -> Glyph {
        self.species_icon(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        let thresholds = self.thresholds();
        if self.grains_per_cubic_metre >= thresholds.high {
            WidgetState::Critical
        } else if self.grains_per_cubic_metre >= thresholds.moderate {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        let full_scale = self.thresholds().gauge_full_scale;
        let logarithmic_share = self.grains_per_cubic_metre.ln_1p() / full_scale.ln_1p();
        Some(Ratio::new::<ratio>(logarithmic_share))
    }

    fn tooltip(&self) -> TooltipBody {
        let thresholds = self.thresholds();
        TooltipBody::sections(&[Section::untitled_dictionary([
            KeyValue::new(
                self.allergen.name(),
                format!("{:.1} grains/m³", self.grains_per_cubic_metre),
            ),
            KeyValue::new(
                "Moderate from",
                format!("{} grains/m³", thresholds.moderate),
            ),
            KeyValue::new("High from", format!("{} grains/m³", thresholds.high)),
        ])])
    }
}

impl BarWidget for Pollen {
    fn name(&self) -> &'static str {
        "Pollen"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Flower(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Flower(Weight::Regular).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[])
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
        let full_scale = grass(0.0).thresholds().gauge_full_scale;
        assert!((gauge_fill(full_scale) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trace_amounts_below_one_grain_still_show() {
        let trace = gauge_fill(0.7);
        assert!(trace > 0.05 && trace < gauge_fill(2.0));
    }
}
