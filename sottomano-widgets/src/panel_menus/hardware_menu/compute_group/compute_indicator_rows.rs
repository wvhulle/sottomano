use chrono::Locale;

use super::{
    ComputeGroup, ComputeInteraction, ComputeSource,
    cpu_clock_label::cpu_clock_indicator,
    cpu_clock_limit_causes::ClockLimitCauses,
    cpu_load_gauge::cpu_usage_indicator,
    cpu_model_identity::cpu_model_indicator,
    cpu_power_draw_label::{cpu_power_hover_card, cpu_power_indicator},
    cpu_temperature_label::temperature_indicator,
    x3d_cache_preference_toggle::x3d_cache_preference_indicator,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        toggle_control::ToggleableSource,
    },
};

const CPU_LOAD_GLYPH: Phosphor = Phosphor::Cpu(Weight::Regular);
const CPU_CLOCK_GLYPH: Phosphor = Phosphor::WaveSine(Weight::Regular);
const CPU_TEMPERATURE_GLYPH: Phosphor = Phosphor::ThermometerSimple(Weight::Regular);
const X3D_CACHE_PREFERENCE_GLYPH: Phosphor = Phosphor::Warehouse(Weight::Regular);

impl ComputeGroup {
    pub(super) fn cpu_load_headline(&self) -> GroupIndicator<'_> {
        self.cpu_load_indicator(self.cpu_load.indicator(
            |held_snapshot| cpu_usage_indicator(held_snapshot),
            CPU_LOAD_GLYPH.into(),
        ))
        .with_headline_glyph(Phosphor::Circuitry(Weight::Regular).into())
    }

    pub(super) fn list_cpu_load<'view>(&'view self, indicators: &mut Vec<GroupIndicator<'view>>) {
        if !self.cpu_load.awaits_first_delivery() {
            indicators.insert(
                0,
                self.cpu_load_indicator(self.cpu_load.indicator(
                    |held_snapshot| cpu_usage_indicator(held_snapshot),
                    CPU_LOAD_GLYPH.into(),
                )),
            );
        }
    }

    pub(super) fn push_cpu_clock_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if self.cpu_clock.awaits_first_delivery() {
            return;
        }
        let clock_indicator = self
            .cpu_clock
            .indicator(cpu_clock_indicator, CPU_CLOCK_GLYPH.into());
        if clock_indicator.panel_visibility == PanelVisibility::Shown {
            rows.push(self.wired_cpu_clock_indicator(clock_indicator));
        }
        if let Some(held_snapshot) = self.cpu_clock.get() {
            let limits_indicator = ClockLimitCauses {
                clock: held_snapshot,
            }
            .indicator();
            if limits_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.cpu_clock_limits_indicator(limits_indicator));
            }
        }
    }

    pub(super) fn push_cpu_temperature_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        clock: Clock,
        locale: Locale,
    ) {
        if !self.cpu_temperature.awaits_first_delivery() {
            let temperature_row = self.cpu_temperature.indicator(
                |held_snapshot| temperature_indicator(held_snapshot, clock, locale),
                CPU_TEMPERATURE_GLYPH.into(),
            );
            if temperature_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.cpu_temperature_indicator(temperature_row));
            }
        }
    }

    pub(super) fn push_cpu_power_draw_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if let Some(power) = self.cpu_power_draw.get() {
            let power_indicator = cpu_power_indicator(power);
            if power_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.cpu_power_draw_indicator(power_indicator));
            }
        }
    }

    pub(super) fn push_cpu_model_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if let Some(hardware) = self.host_hardware.get() {
            let model_indicator = cpu_model_indicator(&hardware.cpu);
            if model_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(
                    model_indicator.with_hover_card(Box::new(move |_| self.cpu_model_card())),
                );
            }
        }
    }

    pub(super) fn push_x3d_cache_preference_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        let subscribed = &self.x3d_cache_preference.sample;
        if subscribed.awaits_first_delivery() {
            return;
        }
        let preference_indicator = self.wired_x3d_cache_preference_indicator(subscribed.indicator(
            x3d_cache_preference_indicator,
            X3D_CACHE_PREFERENCE_GLYPH.into(),
        ));
        let preference_indicator = match subscribed.get() {
            Some(held_snapshot) => preference_indicator.on_switch_press(
                held_snapshot.switch_position(),
                held_snapshot.press_action_text(),
                ComputeInteraction::X3dCachePreferencePressed,
            ),
            None => preference_indicator,
        };
        if preference_indicator.panel_visibility == PanelVisibility::Shown {
            rows.push(preference_indicator);
        }
    }

    fn cpu_load_indicator<'view>(
        &'view self,
        load_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        load_indicator
            .with_hover_card(Box::new(move |_| self.cpu_load_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCpuLoadFetches,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuLoad))
    }

    fn wired_cpu_clock_indicator<'view>(
        &'view self,
        clock_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        clock_indicator
            .with_hover_card(Box::new(move |_| self.cpu_clock_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCpuClockFetch,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
    }

    fn cpu_clock_limits_indicator<'view>(
        &'view self,
        limits_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        limits_indicator
            .with_hover_card(Box::new(move |_| self.cpu_clock_limits_card()))
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
    }

    fn cpu_temperature_indicator<'view>(
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

    fn cpu_power_draw_indicator<'view>(
        &'view self,
        power_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        power_indicator
            .with_hover_card(Box::new(move |_| {
                self.cpu_power_draw
                    .sections_described_by(cpu_power_hover_card)
            }))
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuPowerDraw))
    }

    fn wired_x3d_cache_preference_indicator<'view>(
        &'view self,
        preference_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        preference_indicator
            .with_hover_card(Box::new(move |_| self.x3d_cache_preference_card()))
            .refreshed_by(ComputeInteraction::Refresh(
                ComputeSource::X3dCachePreference,
            ))
    }
}
