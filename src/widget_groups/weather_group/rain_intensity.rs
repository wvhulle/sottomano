use crate::{
    data::source::weather::{Rain, RainOutlook},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        font_awesome::Solid,
        localized_time::TimePart,
        tooltip_body::{Cell, TooltipBody, cells, row},
    },
};

impl BarWidget for Rain {
    fn icon(&self) -> Glyph {
        let peak = self.peak.mm_per_h;
        if peak < 0.5 {
            Solid::Droplet.into()
        } else if peak < 2.5 {
            Solid::CloudRain.into()
        } else if peak < 7.5 {
            Solid::CloudShowersHeavy.into()
        } else {
            Solid::CloudBolt.into()
        }
    }

    fn fallback_icon() -> Glyph {
        Solid::Droplet.into()
    }

    fn label(&self) -> Option<String> {
        match self.outlook {
            RainOutlook::Dry => None,
            RainOutlook::StartsIn(until_wet) => Some(format!("in {}m", until_wet.num_minutes())),
            RainOutlook::StopsIn(until_dry) => Some(format!("for {}m", until_dry.num_minutes())),
            RainOutlook::OutlastsForecast(horizon) => {
                Some(format!("for {}m+", horizon.num_minutes()))
            }
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let now = if self.mm_per_h <= 0.0 {
            "none right now".to_string()
        } else {
            format!("{:.1} mm/h", self.mm_per_h)
        };
        TooltipBody::rows(vec![
            row("Rain", now),
            cells(
                "Peak",
                vec![
                    format!("{:.1} mm/h", self.peak.mm_per_h).into(),
                    Cell::Time {
                        at: self.peak.at.to_utc(),
                        part: TimePart::TimeOfDay,
                    },
                ],
            ),
        ])
    }
}

impl Occasional for Rain {
    fn present(&self) -> bool {
        self.outlook != RainOutlook::Dry
    }
}
