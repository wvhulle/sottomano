use chrono::{DateTime, TimeDelta, Utc};
use color::{AlphaColor, Oklab, Srgb};
use sottomano_config::theme::palette::Palette;

use super::{Accounting, time_intervals::TrailingPeriod};

impl Accounting<'_> {
    #[must_use]
    pub fn recent_switches_to(&self, window: u64, now: DateTime<Utc>) -> usize {
        let recent = TrailingPeriod {
            length: TimeDelta::from_std(self.config.switch_heat_span).unwrap_or(TimeDelta::MAX),
        };
        self.switches_to(window, recent.ending_at(now), now)
    }

    #[must_use]
    pub fn switch_heat_color(
        &self,
        window: u64,
        palette: &Palette,
        now: DateTime<Utc>,
    ) -> Option<AlphaColor<Srgb>> {
        let switches = self.recent_switches_to(window, now);
        (switches > 1).then(|| self.heat_gradient_at(switches, palette))
    }

    #[allow(clippy::cast_precision_loss)]
    fn heat_gradient_at(&self, switches: usize, palette: &Palette) -> AlphaColor<Srgb> {
        let warning = self.config.switch_heat_warning.max(2) as usize;
        let critical = (self.config.switch_heat_critical as usize).max(warning + 1);
        let progress_between = |from: usize, until: usize| {
            (switches.clamp(from, until) - from) as f32 / (until - from) as f32
        };
        if switches < warning {
            blended_in_oklab(
                palette.background,
                palette.warning,
                progress_between(1, warning),
            )
        } else {
            blended_in_oklab(
                palette.warning,
                palette.danger,
                progress_between(warning, critical),
            )
        }
    }
}

fn blended_in_oklab(
    cold: AlphaColor<Srgb>,
    hot: AlphaColor<Srgb>,
    progress: f32,
) -> AlphaColor<Srgb> {
    cold.convert::<Oklab>()
        .lerp_rect(hot.convert::<Oklab>(), progress)
        .convert::<Srgb>()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use color::{AlphaColor, Oklab, Srgb};
    use sottomano_config::{theme::palette::Palette, widget_groups::focus_time::FocusTimeConfig};

    use super::super::{
        FocusLedger,
        test_steps::{at, config, focused_in, no_window_focused},
    };

    fn returned_to_kitty(times: i64) -> FocusLedger {
        let mut ledger = FocusLedger::started_at(at(0));
        for round in 0..times {
            ledger.fold(focused_in(1, "kitty", None), at(round * 2), &config());
            ledger.fold(focused_in(2, "firefox", None), at(round * 2 + 1), &config());
        }
        ledger.fold(no_window_focused(), at(times * 2), &config());
        ledger
    }

    fn two_hour_span_config() -> FocusTimeConfig {
        FocusTimeConfig {
            switch_heat_span: Duration::from_hours(2),
            ..config()
        }
    }

    fn heat_after_returning(times: u32, config: &FocusTimeConfig) -> AlphaColor<Srgb> {
        returned_to_kitty(i64::from(times))
            .accounted(config)
            .switch_heat_color(1, &Palette::default(), at(i64::from(times) * 2))
            .unwrap()
    }

    #[test]
    fn a_window_focused_once_keeps_the_compositor_border() {
        let config = config();
        let ledger = returned_to_kitty(1);
        assert_eq!(
            ledger
                .accounted(&config)
                .switch_heat_color(1, &Palette::default(), at(2)),
            None
        );
    }

    #[test]
    fn the_warning_count_shows_the_warning_color() {
        let config = two_hour_span_config();
        let color = heat_after_returning(config.switch_heat_warning, &config);
        assert_eq!(color.to_rgba8(), Palette::default().warning.to_rgba8());
    }

    #[test]
    fn counts_past_critical_stay_at_the_urgent_color() {
        let config = two_hour_span_config();
        let color = heat_after_returning(config.switch_heat_critical + 3, &config);
        assert_eq!(color.to_rgba8(), Palette::default().danger.to_rgba8());
    }

    #[test]
    fn counts_between_thresholds_blend_both_colors() {
        let config = two_hour_span_config();
        let palette = Palette::default();
        let midway = u32::midpoint(config.switch_heat_warning, config.switch_heat_critical);
        let color = heat_after_returning(midway, &config).to_rgba8();
        assert_ne!(color, palette.warning.to_rgba8());
        assert_ne!(color, palette.danger.to_rgba8());
    }

    #[test]
    fn a_typically_visited_window_stays_closer_to_neutral_than_to_warning() {
        let config = two_hour_span_config();
        let palette = Palette::default();
        let lightness = |color: AlphaColor<Srgb>| color.convert::<Oklab>().components[0];
        let typical = lightness(heat_after_returning(6, &config));
        let neutral = lightness(palette.background);
        let warning = lightness(palette.warning);
        assert!(typical - neutral < warning - typical);
    }

    #[test]
    fn switches_older_than_the_span_cool_down() {
        let config = config();
        let ledger = returned_to_kitty(8);
        assert_eq!(
            ledger
                .accounted(&config)
                .switch_heat_color(1, &Palette::default(), at(60)),
            None
        );
    }
}
