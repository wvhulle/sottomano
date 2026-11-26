mod cpu_tuning_sections;
pub(super) mod power_profile_control;
mod profile_status_sections;

use sottomano_toml_config::Config;

use self::{
    cpu_tuning_sections::cpu_tuning_sections, power_profile_control::PowerProfileInteraction,
};
use super::{PowerBusRequest, PowerGroup, PowerInteraction, PowerRefreshedSource};
use crate::group_indicators::{
    group_indicator::GroupIndicator, indicator_group_content::IndicatorListing,
};

impl PowerGroup {
    pub(super) fn power_profile_listing<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorListing<'view> {
        IndicatorListing::Listed(
            self.power_profile
                .status
                .get()
                .map(|_| self.power_profile_indicator(config))
                .into_iter()
                .collect(),
        )
    }

    fn power_profile_indicator<'view>(&'view self, config: &'view Config) -> GroupIndicator<'view> {
        GroupIndicator::slider(&self.power_profile.control, config, |control_message| {
            PowerInteraction::PowerProfile(PowerProfileInteraction::Control(control_message))
        })
        .with_hover_card(Box::new(move |_| {
            let mut sections = self.power_profile.hover_card();
            sections.extend(self.cpu_tuning.sections_described_by(cpu_tuning_sections));
            sections
        }))
        .fetching_on_hover(
            PowerInteraction::OnBuses(PowerBusRequest::StartCpuTuningFetch),
            PowerInteraction::StopHoverFetches,
        )
        .refreshed_by(PowerInteraction::OnBuses(PowerBusRequest::Refresh(
            PowerRefreshedSource::Profile,
        )))
    }
}
