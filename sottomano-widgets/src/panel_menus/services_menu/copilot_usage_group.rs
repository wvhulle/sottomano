use crate::{
    group_indicators::indicator_group_content::IndicatorListing,
    panel_menus::services_menu::copilot_usage_group::metered_quota_indicator::copilot_metered_indicator,
};
mod cli_spend_indicator;
mod free_quota_indicator;
mod metered_quota_indicator;
mod most_used_model_indicator;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        copilot_usage::{CopilotCcusage, CopilotFree, CopilotMetered},
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{Config, secrets::StoredSecret};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopilotAccountSource {
    Metered,
    Free,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopilotUsageBusRequest {
    Refresh(CopilotAccountSource),
}

#[derive(Debug, Clone, Copy)]
pub enum CopilotUsageInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    RefreshCli,
    OnBuses(CopilotUsageBusRequest),
}

pub struct CopilotUsageGroup {
    metered: SubscribedSnapshot<CopilotMetered>,
    free: SubscribedSnapshot<CopilotFree>,
    cli: SubscribedSnapshot<CopilotCcusage>,
    gh_auth_token: StoredSecret,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum CopilotUsageMessage {
    Metered(SnapshotDelivery<CopilotMetered>),
    Free(SnapshotDelivery<CopilotFree>),
    Cli(SnapshotDelivery<CopilotCcusage>),
}

impl CopilotUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(gh_auth_token: StoredSecret) -> Self {
        Self {
            metered: SubscribedSnapshot::default(),
            free: SubscribedSnapshot::default(),
            cli: SubscribedSnapshot::default(),
            gh_auth_token,
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    fn account(&self, buses: &DesktopBuses) -> Option<OnBus<PathBuf>> {
        let StoredSecret::Present(gh_auth_token) = &self.gh_auth_token else {
            return None;
        };
        Some(OnBus {
            buses: buses.clone(),
            inputs: gh_auth_token.clone(),
        })
    }

    pub fn on_buses(
        &self,
        request: CopilotUsageBusRequest,
        buses: &DesktopBuses,
    ) -> Task<CopilotUsageMessage> {
        let CopilotUsageBusRequest::Refresh(source) = request;
        let Some(account) = self.account(buses) else {
            return Task::none();
        };
        match source {
            CopilotAccountSource::Metered => {
                first_delivery::<CopilotMetered>(account).map(CopilotUsageMessage::Metered)
            }
            CopilotAccountSource::Free => {
                first_delivery::<CopilotFree>(account).map(CopilotUsageMessage::Free)
            }
        }
    }

    pub fn subscription(
        config: &Config,
        working_days: WeekdaySet,
    ) -> Subscription<CopilotUsageMessage> {
        snapshot_subscription::<CopilotCcusage>(cli_budget(config, working_days))
            .map(CopilotUsageMessage::Cli)
    }

    pub fn account_subscription(&self, buses: &DesktopBuses) -> Subscription<CopilotUsageMessage> {
        Subscription::batch(self.account(buses).into_iter().flat_map(|account| {
            [
                snapshot_subscription::<CopilotMetered>(account.clone())
                    .map(CopilotUsageMessage::Metered),
                snapshot_subscription::<CopilotFree>(account).map(CopilotUsageMessage::Free),
            ]
        }))
    }

    pub fn update(
        &mut self,
        message: CopilotUsageMessage,
        clock: Clock,
    ) -> Task<CopilotUsageMessage> {
        match message {
            CopilotUsageMessage::Metered(sample) => self.metered.receive(sample, clock),
            CopilotUsageMessage::Free(sample) => self.free.receive(sample, clock),
            CopilotUsageMessage::Cli(sample) => self.cli.receive(sample, clock),
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: CopilotUsageInteraction,
        config: &Config,
        working_days: WeekdaySet,
    ) -> Task<CopilotUsageMessage> {
        match interaction {
            CopilotUsageInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            CopilotUsageInteraction::RefreshCli => {
                first_delivery::<CopilotCcusage>(cli_budget(config, working_days))
                    .map(CopilotUsageMessage::Cli)
            }
            CopilotUsageInteraction::OnBuses(_) => Task::none(),
        }
    }

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.metered_headline(clock);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::CopilotUsage,
            headline_indicator,
            &self.expansion,
            CopilotUsageInteraction::Expansion,
            [
                self.metered_quota_listing(clock),
                self.free_quota_listing(),
                self.cli_spend_listing(clock),
                IndicatorListing::Listed(self.most_used_model_rows()),
            ]
            .into_iter()
            .collect(),
        )
    }

    fn metered_headline(&self, clock: Clock) -> GroupIndicator<'_> {
        self.wired_metered_indicator(self.metered.indicator(
            |held_snapshot| copilot_metered_indicator(held_snapshot, clock),
            Phosphor::Gauge(Weight::Regular).into(),
        ))
        .with_headline_glyph(SimpleIcon::GitHubCopilot.into())
    }
}

fn cli_budget(config: &Config, working_days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime::new(config, working_days)),
        monthly_limit_usd: config.panel_menus.services.copilot_usage.monthly_budget_usd,
    }
}
