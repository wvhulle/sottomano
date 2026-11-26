mod ccusage_block_indicator;
mod ccusage_periods_indicator;
mod enterprise_spend_indicator;
mod most_used_model_indicator;
mod usage_windows_indicator;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        claude_usage::{
            ClaudeCcusageBlock, ClaudeCcusageModels, ClaudeCcusagePeriods, ClaudeEnterpriseAccount,
            ClaudeEnterpriseSpend, ClaudeLocalLogs, ClaudeOrganization, ClaudeUsageWindows,
        },
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{Config, secrets::StoredSecret};

use self::usage_windows_indicator::claude_usage_windows_indicator;
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
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeLocalSource {
    CcusagePeriods,
    CcusageBlock,
    CcusageModels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeUsageBusRequest {
    StartOrganizationFetch,
    RefreshUsageWindows,
    RefreshEnterpriseSpend,
}

#[derive(Debug, Clone, Copy)]
pub enum ClaudeUsageInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    StopHoverFetches,
    Refresh(ClaudeLocalSource),
    OnBuses(ClaudeUsageBusRequest),
}

pub struct ClaudeUsageGroup {
    usage_windows: SubscribedSnapshot<ClaudeUsageWindows>,
    enterprise_spend: SubscribedSnapshot<ClaudeEnterpriseSpend>,
    ccusage_periods: SubscribedSnapshot<ClaudeCcusagePeriods>,
    ccusage_block: SubscribedSnapshot<ClaudeCcusageBlock>,
    ccusage_models: SubscribedSnapshot<ClaudeCcusageModels>,
    oauth_credentials: StoredSecret,
    organization: HoverFetchedSnapshot<ClaudeOrganization>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum ClaudeUsageMessage {
    UsageWindows(Box<SnapshotDelivery<ClaudeUsageWindows>>),
    EnterpriseSpend(SnapshotDelivery<ClaudeEnterpriseSpend>),
    OrganizationFetched(SnapshotDelivery<ClaudeOrganization>),
    CcusagePeriods(SnapshotDelivery<ClaudeCcusagePeriods>),
    CcusageBlock(SnapshotDelivery<ClaudeCcusageBlock>),
    CcusageModels(SnapshotDelivery<ClaudeCcusageModels>),
}

impl ClaudeUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(oauth_credentials: StoredSecret) -> Self {
        Self {
            usage_windows: SubscribedSnapshot::default(),
            enterprise_spend: SubscribedSnapshot::default(),
            ccusage_periods: SubscribedSnapshot::default(),
            ccusage_block: SubscribedSnapshot::default(),
            ccusage_models: SubscribedSnapshot::default(),
            oauth_credentials,
            organization: HoverFetchedSnapshot::default(),
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    pub fn on_buses(
        &mut self,
        request: ClaudeUsageBusRequest,
        config: &Config,
        working_days: WeekdaySet,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<ClaudeUsageMessage> {
        let StoredSecret::Present(credentials) = self.oauth_credentials.clone() else {
            return Task::none();
        };
        match request {
            ClaudeUsageBusRequest::StartOrganizationFetch => self.organization.start_hover_fetch(
                OnBus {
                    buses: buses.clone(),
                    inputs: credentials,
                },
                ClaudeUsageMessage::OrganizationFetched,
                config,
                clock,
            ),
            ClaudeUsageBusRequest::RefreshUsageWindows => {
                first_delivery::<ClaudeUsageWindows>(OnBus {
                    buses: buses.clone(),
                    inputs: credentials,
                })
                .map(|sample| ClaudeUsageMessage::UsageWindows(Box::new(sample)))
            }
            ClaudeUsageBusRequest::RefreshEnterpriseSpend => {
                first_delivery::<ClaudeEnterpriseSpend>(enterprise_account(
                    credentials,
                    config,
                    working_days,
                    buses,
                ))
                .map(ClaudeUsageMessage::EnterpriseSpend)
            }
        }
    }

    fn local_logs(&self, config: &Config) -> ClaudeLocalLogs {
        ClaudeLocalLogs {
            budget: SpendBudget {
                pacing: SpendPacing::CalendarTime,
                monthly_limit_usd: config.panel_menus.services.claude_usage.monthly_budget_usd,
            },
            credentials: self.oauth_credentials.clone(),
        }
    }

    fn refresh(&self, source: ClaudeLocalSource, config: &Config) -> Task<ClaudeUsageMessage> {
        match source {
            ClaudeLocalSource::CcusagePeriods => {
                first_delivery::<ClaudeCcusagePeriods>(self.local_logs(config))
                    .map(ClaudeUsageMessage::CcusagePeriods)
            }
            ClaudeLocalSource::CcusageModels => {
                first_delivery::<ClaudeCcusageModels>(self.oauth_credentials.clone())
                    .map(ClaudeUsageMessage::CcusageModels)
            }
            ClaudeLocalSource::CcusageBlock => {
                first_delivery::<ClaudeCcusageBlock>(self.local_logs(config))
                    .map(ClaudeUsageMessage::CcusageBlock)
            }
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        working_days: WeekdaySet,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<ClaudeUsageMessage> {
        let local_logs = self.local_logs(config);
        let mut subscriptions = vec![
            snapshot_subscription::<ClaudeCcusageBlock>(local_logs.clone())
                .map(ClaudeUsageMessage::CcusageBlock),
            snapshot_subscription::<ClaudeCcusagePeriods>(local_logs)
                .map(ClaudeUsageMessage::CcusagePeriods),
            snapshot_subscription::<ClaudeCcusageModels>(self.oauth_credentials.clone())
                .map(ClaudeUsageMessage::CcusageModels),
        ];
        if let StoredSecret::Present(key) = &self.oauth_credentials
            && let Some(buses) = buses
        {
            subscriptions.extend([
                snapshot_subscription::<ClaudeUsageWindows>(OnBus {
                    buses: buses.clone(),
                    inputs: key.clone(),
                })
                .map(|sample| ClaudeUsageMessage::UsageWindows(Box::new(sample))),
                snapshot_subscription::<ClaudeEnterpriseSpend>(enterprise_account(
                    key.clone(),
                    config,
                    working_days,
                    buses,
                ))
                .map(ClaudeUsageMessage::EnterpriseSpend),
            ]);
        }
        Subscription::batch(subscriptions)
    }

    pub fn update(
        &mut self,
        message: ClaudeUsageMessage,
        clock: Clock,
    ) -> Task<ClaudeUsageMessage> {
        match message {
            ClaudeUsageMessage::UsageWindows(sample) => {
                self.usage_windows.receive(*sample, clock);
            }
            ClaudeUsageMessage::EnterpriseSpend(sample) => {
                self.enterprise_spend.receive(sample, clock);
            }
            ClaudeUsageMessage::OrganizationFetched(fetched) => {
                self.organization.receive(fetched, clock);
            }
            ClaudeUsageMessage::CcusagePeriods(sample) => {
                self.ccusage_periods.receive(sample, clock);
            }
            ClaudeUsageMessage::CcusageBlock(sample) => {
                self.ccusage_block.receive(sample, clock);
            }
            ClaudeUsageMessage::CcusageModels(sample) => {
                self.ccusage_models.receive(sample, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: ClaudeUsageInteraction,
        config: &Config,
    ) -> Task<ClaudeUsageMessage> {
        match interaction {
            ClaudeUsageInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            ClaudeUsageInteraction::StopHoverFetches => {
                self.organization.stop();
                Task::none()
            }
            ClaudeUsageInteraction::Refresh(source) => self.refresh(source, config),
            ClaudeUsageInteraction::OnBuses(_) => Task::none(),
        }
    }
}

fn enterprise_account(
    credentials: PathBuf,
    config: &Config,
    working_days: WeekdaySet,
    buses: &DesktopBuses,
) -> OnBus<ClaudeEnterpriseAccount> {
    OnBus {
        buses: buses.clone(),
        inputs: ClaudeEnterpriseAccount {
            credentials,
            working_time: WorkingTime::new(config, working_days),
        },
    }
}

impl ClaudeUsageGroup {
    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.usage_windows_headline(clock);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::ClaudeUsage,
            headline_indicator,
            &self.expansion,
            ClaudeUsageInteraction::Expansion,
            [
                self.usage_windows_listing(clock),
                self.enterprise_spend_listing(clock),
                self.ccusage_periods_listing(clock),
                self.ccusage_block_listing(),
                IndicatorListing::Listed(self.most_used_model_rows()),
            ]
            .into_iter()
            .collect(),
        )
    }

    fn usage_windows_headline(&self, clock: Clock) -> GroupIndicator<'_> {
        self.usage_windows_indicator(self.usage_windows.indicator(
            |held_snapshot| claude_usage_windows_indicator(held_snapshot, clock),
            Phosphor::Gauge(Weight::Regular).into(),
        ))
        .with_headline_glyph(SimpleIcon::Claude.into())
    }
}
