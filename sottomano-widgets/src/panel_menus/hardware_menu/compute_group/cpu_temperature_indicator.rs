use std::{iter::once, time::Duration};

use chrono::{DateTime, Locale, Utc};
use corredo::changing_text::escape_markup;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{
        cpu_temperature::{
            CoreTemperature, CpuCoreTemperatures, CriticalOnset, Temperature, ThrottleLimit,
            cpu_thresholds,
        },
        intel_thermal_throttle_counters::{ThermalThrottleActivity, ThrottledTime},
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
    ComputeGroup, ComputeInteraction, ComputeSource,
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCoreSample, cpu_ranges},
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{
        Section, SectionTitle, dictionary_table, headed_table,
        hover_card_figure_payloads::ThreadCell,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        ongoing_toast::{
            IndicatorAlerts, NotificationBody, NotificationProgress, StateToast, definition_list,
            moment_text,
        },
        value_cells::{LabeledValue, ValueCell},
    },
};

const CPU_TEMPERATURE_GLYPH: Phosphor = Phosphor::ThermometerSimple(Weight::Regular);

const SUSTAINED_CRITICAL: Duration = Duration::from_secs(30);

impl PerCoreSample for CoreTemperature {
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

fn temperature_state(temperature: &Temperature, clock: Clock) -> IndicatorState {
    let critical_sustained = matches!(
        temperature.critical_since,
        CriticalOnset::CriticalSince(since) if clock
            .wall
            .duration_since(since)
            .is_ok_and(|held| held >= SUSTAINED_CRITICAL)
    );
    match temperature.thresholds().level(temperature.hottest()) {
        Severity::Critical if !critical_sustained => Severity::Warning,
        level => level,
    }
    .into()
}

fn temperature_toast_text(temperature: &Temperature, clock: Clock, locale: Locale) -> String {
    let thresholds = temperature.thresholds();
    let sensor = temperature.hottest_sample().map(|sample| {
        (
            "Sensor",
            escape_markup(format!("{} {}", sample.chip, sample.label)).into_owned(),
        )
    });
    let above_since = match temperature.critical_since {
        CriticalOnset::CriticalSince(since) => Some((
            "Above critical since",
            moment_text(DateTime::<Utc>::from(since), clock.utc(), locale),
        )),
        CriticalOnset::BelowCritical => None,
    };
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

pub(super) fn temperature_indicator(
    temperature: &Temperature,
    clock: Clock,
    locale: Locale,
) -> GroupIndicator<'_> {
    let state = temperature_state(temperature, clock);
    GroupIndicator::glyph_only(
        Temperature::NAME,
        Phosphor::ThermometerSimple(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(celsius_text(temperature.hottest())),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        state_toast: StateToast::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "CPU overheating",
            body: NotificationBody::Text(temperature_toast_text(temperature, clock, locale)),
            progress: NotificationProgress::Unmeasured,
        },
        ..IndicatorAlerts::default()
    })
    .with_hover_card(Box::new(move |_| temperature_hover_card(temperature)))
}

pub(super) fn temperature_hover_card(temperature: &Temperature) -> Vec<Section> {
    let mut chips: Vec<&str> = temperature
        .samples
        .iter()
        .map(|sample| sample.chip.as_str())
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
                    Quantity::count(temperature.samples.len()),
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
                match temperature.throttle_limit {
                    ThrottleLimit::ThrottlesAt(throttle) => Some(LabeledValue::new(
                        "Throttles at",
                        Quantity::temperature(throttle),
                    )),
                    ThrottleLimit::Unreported => None,
                },
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

struct ThrottleScopeRow {
    scope: &'static str,
    cpus: String,
    throttled_time: ThrottledTime,
}

impl From<ThrottleScopeRow> for TableRecord<3> {
    fn from(row: ThrottleScopeRow) -> Self {
        TableRecord::with_cells(
            row.scope,
            [
                row.cpus.into(),
                ValueCell::duration(row.throttled_time.throttled),
                Quantity::share(row.throttled_time.share_of_window).into(),
            ],
        )
        .level(Severity::Warning)
    }
}

pub(super) fn hardware_throttling_sections(activity: &ThermalThrottleActivity) -> Vec<Section> {
    let ThermalThrottleActivity::Throttling {
        package,
        throttled_cores,
    } = activity
    else {
        return Vec::new();
    };
    let package_row = ThrottleScopeRow {
        scope: "Package",
        cpus: "all".to_owned(),
        throttled_time: *package,
    };
    let core_row = throttled_cores
        .iter()
        .map(|core| core.throttled_time)
        .max_by_key(|throttled_time| throttled_time.throttled)
        .map(|longest| ThrottleScopeRow {
            scope: "Core",
            cpus: cpu_ranges(throttled_cores.iter().map(|core| core.cpu)),
            throttled_time: longest,
        });
    vec![Section::new(
        SectionTitle::captioned(
            "Hardware throttling",
            "Time in the recent window during which the CPU slowed its own clock because it ran \
             hot, and that time as a share of the window.",
        ),
        headed_table(
            [
                ColumnHeading::titled("CPUs"),
                ColumnHeading::titled("Throttled"),
                ColumnHeading::titled("Share"),
            ],
            once(package_row).chain(core_row),
        ),
    )]
}

impl ChipletLiveState<'_> {
    pub fn core_temperature_sections(&self, temperatures: &CpuCoreTemperatures) -> Vec<Section> {
        vec![
            CoreGridSection {
                topology: &temperatures.topology,
                samples: &temperatures.cores,
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
                        .map(|sample| sample.temperature)
                        .reduce(ThermodynamicTemperature::max)
                        .map(|hottest| LabeledValue::new("Hottest", Quantity::temperature(hottest)))
                        .into_iter()
                        .collect()
                },
            ),
        ]
    }
}

impl ComputeGroup {
    pub(super) fn cpu_temperature_listing(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> IndicatorListing<'_> {
        self.cpu_temperature
            .indicator_listing(
                |held_snapshot| temperature_indicator(held_snapshot, clock, locale).when_shown(),
                CPU_TEMPERATURE_GLYPH.into(),
            )
            .map_indicators(|temperature_row| self.wired_cpu_temperature_indicator(temperature_row))
    }

    fn wired_cpu_temperature_indicator<'view>(
        &'view self,
        temperature_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        temperature_row
            .with_hover_card(Box::new(move |_| self.cpu_temperature_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCoreTemperaturesFetch,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuTemperature))
    }

    fn cpu_temperature_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self
            .cpu_temperature
            .sections_described_by(temperature_hover_card);
        sections.extend(self.cpu_clock.sections_described_by(|cpu_clock| {
            hardware_throttling_sections(&cpu_clock.thermal_throttle_activity)
        }));
        sections.extend(
            self.core_temperatures
                .sections_described_by(|temperatures| live.core_temperature_sections(temperatures)),
        );
        sections
    }
}

#[cfg(test)]
mod tests {

    use sottomano_desktop_sources::cpu_memory_compute::{
        cpu_temperature::TemperatureSample, temperature_thresholds::celsius,
    };
    use sottomano_toml_config::panel_menus::hardware_menu::temperature_sensors::ThresholdOverrides;

    use super::*;

    fn hot_indicator(critical_since: CriticalOnset) -> Temperature {
        Temperature {
            samples: vec![TemperatureSample {
                chip: "k10temp".into(),
                label: "Tccd1".into(),
                temperature: celsius(90.0),
                limit: ThrottleLimit::Unreported,
            }],
            throttle_limit: ThrottleLimit::Unreported,
            overrides: ThresholdOverrides::default(),
            critical_since,
        }
    }

    #[test]
    fn critical_heat_counts_as_warning_until_sustained() {
        let clock = Clock::real();
        let fresh = hot_indicator(CriticalOnset::CriticalSince(clock.wall));
        assert_eq!(temperature_state(&fresh, clock), IndicatorState::Warning);
        let sustained = hot_indicator(CriticalOnset::CriticalSince(
            clock.wall - SUSTAINED_CRITICAL,
        ));
        assert_eq!(
            temperature_state(&sustained, clock),
            IndicatorState::Critical
        );
    }
}
