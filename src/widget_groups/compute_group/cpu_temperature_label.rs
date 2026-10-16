use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, Locale};
use uom::si::{
    f64::{Ratio, ThermodynamicTemperature},
    ratio::ratio,
    thermodynamic_temperature::degree_celsius,
};

use super::{
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCore},
};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::temperature::{
            CoreTemperature, CpuCoreTemperatures, Temperature, cpu_thresholds,
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        localized_time::TimePart,
        ongoing_toast::OngoingBody,
        phosphor::Phosphor,
        temperature_thresholds::celsius_text,
        tooltip_body::{KeyValue, Section, ThreadCell, TooltipBody},
        widget_state::WidgetState,
    },
};

const SUSTAINED_CRITICAL: Duration = Duration::from_secs(30);

impl PerCore for CoreTemperature {
    fn core(&self) -> u32 {
        self.core
    }

    fn thread_cell(&self) -> ThreadCell {
        let thresholds = cpu_thresholds(self.limit, self.overrides);
        ThreadCell {
            fill: Ratio::new::<ratio>(
                self.temperature.get::<degree_celsius>()
                    / thresholds.critical.get::<degree_celsius>(),
            ),
            level: thresholds.level(self.temperature),
        }
    }
}

impl Temperature {
    fn throttle_text(&self) -> Option<String> {
        self.throttle_limit.map(celsius_text)
    }

    fn critical_sustained(&self) -> bool {
        self.critical_since.is_some_and(|since| {
            SystemTime::now()
                .duration_since(since)
                .is_ok_and(|held| held >= SUSTAINED_CRITICAL)
        })
    }
}

impl BarWidget for Temperature {
    fn name(&self) -> &'static str {
        "CPU temperature"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ThermometerSimple(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        match self.thresholds().level(self.hottest()) {
            Severity::Critical if !self.critical_sustained() => Severity::Warning,
            level => level,
        }
        .into()
    }

    fn label(&self) -> Option<String> {
        Some(celsius_text(self.hottest()))
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "CPU overheating",
        }
    }

    fn ongoing_toast_body(&self, locale: Locale) -> OngoingBody {
        let thresholds = self.thresholds();
        let hottest = self.hottest_reading().map_or_else(
            || format!("CPU at {}", celsius_text(self.hottest())),
            |reading| {
                format!(
                    "{} {} at {}",
                    reading.chip,
                    reading.label,
                    celsius_text(reading.temperature)
                )
            },
        );
        let since = self
            .critical_since
            .map(|since| {
                format!(
                    ", above {} since {}",
                    celsius_text(thresholds.critical),
                    TimePart::TimeOfDay.format(&DateTime::<Local>::from(since), locale)
                )
            })
            .unwrap_or_default();
        OngoingBody::Text(format!(
            "{hottest}{since}\nCritical threshold {}",
            thresholds.basis.text(thresholds.critical)
        ))
    }

    fn tooltip(&self) -> TooltipBody {
        let mut chips: Vec<&str> = self.readings.iter().map(|r| r.chip.as_str()).collect();
        chips.dedup();
        let thresholds = self.thresholds();
        let summary = Section::dictionary(
            [
                Some(KeyValue::new("Chips", chips.join(", "))),
                Some(KeyValue::new("Samples", self.readings.len().to_string())),
                self.lowest()
                    .map(|lowest| KeyValue::new("Min", celsius_text(lowest))),
                Some(KeyValue::new("Avg", celsius_text(self.average()))),
                Some(KeyValue::new("Max", celsius_text(self.hottest()))),
            ]
            .into_iter()
            .flatten(),
        );
        let threshold_section = Section::dictionary(
            [
                self.throttle_text()
                    .map(|throttle| KeyValue::new("Throttles at", throttle)),
                Some(KeyValue::new(
                    "Warns above",
                    celsius_text(thresholds.warning),
                )),
                Some(KeyValue::new("Critical above", thresholds.critical_text())),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Thresholds");
        TooltipBody::sections([summary, threshold_section])
    }
}

impl CpuCoreTemperatures {
    pub fn sections(&self, live: &ChipletLiveState) -> Vec<Section> {
        vec![
            CoreGridSection {
                topology: &self.topology,
                readings: &self.cores,
            }
            .section(
                |group| live.title(group),
                |group, members| {
                    members
                        .iter()
                        .map(|reading| reading.temperature)
                        .reduce(ThermodynamicTemperature::max)
                        .map(|hottest| KeyValue::new("Hottest", celsius_text(hottest)))
                        .into_iter()
                        .chain(live.facts(group))
                        .collect()
                },
            )
            .titled((
                "Chiplets".to_owned(),
                "Each square is one physical core in core-number order, not die position, split \
                 into its hardware threads; bar height shows the core's sensor against the \
                 critical threshold."
                    .to_owned(),
            )),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::widget_groups::temperature_sensors::ThresholdOverrides,
        data::source::sysfs_sensors::temperature::TempReading,
        widgets::temperature_thresholds::celsius,
    };

    fn hot_reading(critical_since: Option<SystemTime>) -> Temperature {
        Temperature {
            readings: vec![TempReading {
                chip: "k10temp".into(),
                label: "Tccd1".into(),
                temperature: celsius(90.0),
                limit: None,
            }],
            throttle_limit: None,
            overrides: ThresholdOverrides::default(),
            critical_since,
        }
    }

    #[test]
    fn critical_heat_counts_as_warning_until_sustained() {
        let fresh = hot_reading(Some(SystemTime::now()));
        assert_eq!(fresh.state(), WidgetState::Warning);
        let sustained = hot_reading(Some(SystemTime::now() - SUSTAINED_CRITICAL));
        assert_eq!(sustained.state(), WidgetState::Critical);
    }
}
