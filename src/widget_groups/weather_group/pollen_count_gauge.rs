use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    data::source::weather::{Allergen, AllergenConcentration, Pollen},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

const fn allergen_icon(allergen: Allergen, weight: Weight) -> Phosphor {
    match allergen {
        Allergen::Alder => Phosphor::Leaf(weight),
        Allergen::Birch => Phosphor::Tree(weight),
        Allergen::Grass => Phosphor::Plant(weight),
        Allergen::Mugwort => Phosphor::Clover(weight),
        Allergen::Olive => Phosphor::TreeEvergreen(weight),
        Allergen::Ragweed => Phosphor::Flower(weight),
    }
}

impl BarWidget for AllergenConcentration {
    fn icon(&self) -> Glyph {
        allergen_icon(self.allergen, self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        let thresholds = self.allergen.thresholds();
        if self.grains_per_cubic_metre >= thresholds.high {
            WidgetState::Critical
        } else if self.grains_per_cubic_metre >= thresholds.moderate {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        let full_scale = self.allergen.thresholds().gauge_full_scale;
        let logarithmic_share = self.grains_per_cubic_metre.ln_1p() / full_scale.ln_1p();
        Some(Ratio::new::<ratio>(logarithmic_share))
    }

    fn tooltip(&self) -> TooltipBody {
        let thresholds = self.allergen.thresholds();
        TooltipBody::rows(vec![
            row(
                self.allergen.name(),
                format!("{:.1} grains/m³", self.grains_per_cubic_metre),
            ),
            row(
                "Moderate from",
                format!("{} grains/m³", thresholds.moderate),
            ),
            row("High from", format!("{} grains/m³", thresholds.high)),
        ])
    }
}

impl BarWidget for Pollen {
    fn icon(&self) -> Glyph {
        Phosphor::Flower(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Flower(Weight::Regular).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gauge_fill(grains_per_cubic_metre: f64) -> f64 {
        AllergenConcentration {
            allergen: Allergen::Grass,
            grains_per_cubic_metre,
        }
        .gauge()
        .expect("allergens always have a gauge")
        .get::<ratio>()
    }

    #[test]
    fn gauge_is_empty_without_pollen_and_full_at_full_scale() {
        assert!(gauge_fill(0.0).abs() < f64::EPSILON);
        let full_scale = Allergen::Grass.thresholds().gauge_full_scale;
        assert!((gauge_fill(full_scale) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trace_amounts_below_one_grain_still_show() {
        let trace = gauge_fill(0.7);
        assert!(trace > 0.05 && trace < gauge_fill(2.0));
    }
}
