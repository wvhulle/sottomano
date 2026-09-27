use crate::{
    data::source::weather::Rain,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
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

    fn tooltip(&self) -> TooltipBody {
        let now = if self.mm_per_h <= 0.0 {
            "none right now".to_string()
        } else {
            format!("{:.1} mm/h", self.mm_per_h)
        };
        TooltipBody::rows(vec![
            row("Rain", now),
            row(
                "Peak",
                format!("{:.1} mm/h at {}", self.peak.mm_per_h, self.peak.at),
            ),
        ])
    }
}

impl Occasional for Rain {
    fn present(&self) -> bool {
        self.peak.mm_per_h > 0.0
    }
}
