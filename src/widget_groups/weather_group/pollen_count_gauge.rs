use crate::{
    data::source::weather::{Allergen, Birch, Grass, Pollen},
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Occasional, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

pub trait AllergenIcon: Allergen {
    const ICON: Solid;
}

impl AllergenIcon for Grass {
    const ICON: Solid = Solid::Seedling;
}

impl AllergenIcon for Birch {
    const ICON: Solid = Solid::Tree;
}

impl<A: AllergenIcon> BarWidget for Pollen<A> {
    fn icon(&self) -> Glyph {
        A::ICON.into()
    }

    fn level(&self) -> Level {
        let [moderate, high, _] = A::THRESHOLDS;
        if self.value >= high {
            Level::Critical
        } else if self.value >= moderate {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        let cap = A::THRESHOLDS[2];
        let percent = if self.value < 1.0 {
            0
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let scaled = (self.value.ln() / cap.ln() * 100.0).round() as i64;
            scaled.clamp(0, 100)
        };
        Some(Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut all = self.all.clone();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        TooltipBody::rows(
            all.into_iter()
                .map(|(label, value)| row(label, format!("{value:.0} grains/m³")))
                .collect(),
        )
    }
}

impl<A: AllergenIcon + Clone + PartialEq + Send + Sync + 'static> Occasional for Pollen<A> {
    fn present(&self) -> bool {
        self.value > 0.0
    }
}
