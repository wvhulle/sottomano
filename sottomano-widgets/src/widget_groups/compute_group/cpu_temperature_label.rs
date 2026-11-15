use std::time::Duration;

use chrono::{DateTime, Locale, Utc};
use quick_xml::escape::escape;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{
        cpu_temperature::{CoreTemperature, CpuCoreTemperatures, Temperature, cpu_thresholds},
        temperature_thresholds::celsius_text,
    },
};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{Ratio, ThermodynamicTemperature},
    ratio::ratio,
    thermodynamic_temperature::degree_celsius,
};

use super::{
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCoreReading},
};
use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{
        Section, SectionTitle, dictionary_table, hover_card_figure_payloads::ThreadCell,
    },
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{
            ReadingAlerts, StateToast, ToastBody, ToastProgress, definition_list, moment_text,
        },
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const SUSTAINED_CRITICAL: Duration = Duration::from_secs(30);

impl PerCoreReading for CoreTemperature {
    fn core_index(&self) -> u32 {
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

fn temperature_state(temperature: &Temperature, clock: Clock) -> ReadingDisplayState {
    let critical_sustained = temperature.critical_since.is_some_and(|since| {
        clock
            .wall
            .duration_since(since)
            .is_ok_and(|held| held >= SUSTAINED_CRITICAL)
    });
    match temperature.thresholds().level(temperature.hottest()) {
        Severity::Critical if !critical_sustained => Severity::Warning,
        level => level,
    }
    .into()
}

fn temperature_toast_text(temperature: &Temperature, clock: Clock, locale: Locale) -> String {
    let thresholds = temperature.thresholds();
    let sensor = temperature.hottest_reading().map(|reading| {
        (
            "Sensor",
            escape(format!("{} {}", reading.chip, reading.label)).into_owned(),
        )
    });
    let above_since = temperature.critical_since.map(|since| {
        (
            "Above critical since",
            moment_text(DateTime::<Utc>::from(since), clock.utc(), locale),
        )
    });
    definition_list(
        sensor
            .into_iter()
            .chain([("Temperature", celsius_text(temperature.hottest()))])
            .chain(above_since)
            .chain([(
                "Critical threshold",
                thresholds.basis.text(thresholds.critical),
            )]),
    )
}

pub(super) fn temperature_reading_row(
    temperature: &Temperature,
    clock: Clock,
    locale: Locale,
) -> ReadingRow<'_> {
    let state = temperature_state(temperature, clock);
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(celsius_text(temperature.hottest())),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        alerts: ReadingAlerts {
            state_toast: StateToast::NotifyFromSeverity {
                minimum_severity: Severity::Critical,
                summary_text: "CPU overheating",
                body: ToastBody::Text(temperature_toast_text(temperature, clock, locale)),
                progress: ToastProgress::Unmeasured,
            },
            ..ReadingAlerts::default()
        },
        ..ReadingRow::glyph_only(
            Temperature::NAME,
            Phosphor::ThermometerSimple(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| temperature_hover_card(temperature, card_clock)),
        )
    }
}

pub(super) fn temperature_hover_card(temperature: &Temperature, _clock: Clock) -> Vec<Section> {
    let mut chips: Vec<&str> = temperature
        .readings
        .iter()
        .map(|reading| reading.chip.as_str())
        .collect();
    chips.dedup();
    let thresholds = temperature.thresholds();
    let summary = Section::new(
        "Sensors",
        dictionary_table(
            [
                Some(LabeledValue::new("Chips", chips.join(", "))),
                Some(LabeledValue::new(
                    "Samples",
                    Quantity::count(temperature.readings.len()),
                )),
                temperature
                    .lowest()
                    .map(|lowest| LabeledValue::new("Min", Quantity::temperature(lowest))),
                Some(LabeledValue::new(
                    "Avg",
                    Quantity::temperature(temperature.average()),
                )),
                Some(LabeledValue::new(
                    "Max",
                    Quantity::temperature(temperature.hottest()),
                )),
            ]
            .into_iter()
            .flatten(),
        ),
    );
    let threshold_section = Section::new(
        "Thresholds",
        dictionary_table(
            [
                temperature.throttle_limit.map(|throttle| {
                    LabeledValue::new("Throttles at", Quantity::temperature(throttle))
                }),
                Some(LabeledValue::new(
                    "Warns above",
                    Quantity::temperature(thresholds.warning),
                )),
                Some(LabeledValue::new(
                    "Critical above",
                    thresholds.critical_text(),
                )),
            ]
            .into_iter()
            .flatten(),
        ),
    );
    vec![summary, threshold_section]
}

impl ChipletLiveState<'_> {
    pub fn core_temperature_sections(&self, temperatures: &CpuCoreTemperatures) -> Vec<Section> {
        vec![
            CoreGridSection {
                topology: &temperatures.topology,
                readings: &temperatures.cores,
            }
            .section(
                SectionTitle::captioned(
                    "Cores",
                    "Each square is one physical core in core-number order, not die position, \
                     split into its hardware threads; bar height shows the core's sensor against \
                     the critical threshold.",
                ),
                |group| self.title(group),
                |_, members| {
                    members
                        .iter()
                        .map(|reading| reading.temperature)
                        .reduce(ThermodynamicTemperature::max)
                        .map(|hottest| LabeledValue::new("Hottest", Quantity::temperature(hottest)))
                        .into_iter()
                        .collect()
                },
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use sottomano_desktop_sources::cpu_memory_compute::{
        cpu_temperature::TempReading, temperature_thresholds::celsius,
    };
    use sottomano_toml_config::widget_groups::temperature_sensors::ThresholdOverrides;

    use super::*;

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
        let clock = Clock::real();
        let fresh = hot_reading(Some(clock.wall));
        assert_eq!(
            temperature_state(&fresh, clock),
            ReadingDisplayState::Warning
        );
        let sustained = hot_reading(Some(clock.wall - SUSTAINED_CRITICAL));
        assert_eq!(
            temperature_state(&sustained, clock),
            ReadingDisplayState::Critical
        );
    }
}
