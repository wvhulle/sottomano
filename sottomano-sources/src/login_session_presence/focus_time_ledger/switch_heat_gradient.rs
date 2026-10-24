use chrono::{DateTime, TimeDelta, Utc};
use color::{AlphaColor, Oklab, Srgb};
use sottomano_config::theme::palette::Palette;

use super::{Accounting, recorded_spans::FocusScope, time_intervals::Interval};

impl Accounting<'_> {
    #[must_use]
    pub fn recent_switches_to(&self, window: u64, now: DateTime<Utc>) -> usize {
        let span = TimeDelta::from_std(self.config.switch_heat_span).unwrap_or(TimeDelta::MAX);
        let recent = Interval {
            from: now
                .checked_sub_signed(span)
                .unwrap_or(DateTime::<Utc>::MIN_UTC),
            until: now,
        };
        self.switches_to(&FocusScope::Window(window), recent, now)
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
            blended_in_oklab(palette.focus, palette.warning, progress_between(1, warning))
        } else {
            blended_in_oklab(
                palette.warning,
                palette.urgent,
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

    use sottomano_config::{theme::palette::Palette, widget_groups::focus_time::FocusTimeConfig};

    use super::super::{
        FocusLedger,
        test_steps::{at, config, focused_in, no_window_focused},
    };

    fn returned_to_kitty(times: i64) -> FocusLedger {
        (0..times)
            .fold(FocusLedger::started_at(at(0)), |ledger, round| {
                ledger
                    .fold(focused_in(1, "kitty", None), at(round * 2))
                    .ledger
                    .fold(focused_in(2, "firefox", None), at(round * 2 + 1))
                    .ledger
            })
            .fold(no_window_focused(), at(times * 2))
            .ledger
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
        let config = config();
        let palette = Palette::default();
        let ledger = returned_to_kitty(i64::from(config.switch_heat_warning));
        let color = ledger
            .accounted(&config)
            .switch_heat_color(1, &palette, at(16))
            .unwrap();
        assert_eq!(color.to_rgba8(), palette.warning.to_rgba8());
    }

    #[test]
    fn counts_past_critical_stay_at_the_urgent_color() {
        let config = FocusTimeConfig {
            switch_heat_span: Duration::from_hours(2),
            ..config()
        };
        let palette = Palette::default();
        let ledger = returned_to_kitty(i64::from(config.switch_heat_critical) + 3);
        let color = ledger
            .accounted(&config)
            .switch_heat_color(1, &palette, at(46))
            .unwrap();
        assert_eq!(color.to_rgba8(), palette.urgent.to_rgba8());
    }

    #[test]
    fn counts_between_thresholds_blend_both_colors() {
        let config = config();
        let palette = Palette::default();
        let ledger = returned_to_kitty(14);
        let color = ledger
            .accounted(&config)
            .switch_heat_color(1, &palette, at(28))
            .unwrap()
            .to_rgba8();
        assert_ne!(color, palette.warning.to_rgba8());
        assert_ne!(color, palette.urgent.to_rgba8());
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
