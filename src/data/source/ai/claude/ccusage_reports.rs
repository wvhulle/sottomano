use std::path::PathBuf;

use chrono::{DateTime, Local, Utc};
use iced::futures::Stream;

use super::{Error, local_log_polls};
use crate::data::source::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    ai::{
        ccusage::{self, Agent, BillingBlock, POLL_INTERVAL, RecentUsage},
        quota::Quota,
        working_time_forecast::SpendBudget,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusagePeriods {
    pub recent: RecentUsage,
    pub budget: SpendBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaudeLocalLogs {
    pub budget: SpendBudget,
    pub credentials: Option<PathBuf>,
}

impl ClaudeCcusagePeriods {
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.recent
            .month
            .budget_quota(
                "Claude Code monthly budget",
                &self.budget,
                now.with_timezone(&Local),
            )
            .into_iter()
            .collect()
    }
}

impl ReadingSource for ClaudeCcusagePeriods {
    type Error = Error;
    type Key = ClaudeLocalLogs;

    const NAME: &'static str = "claude-ccusage-periods";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(POLL_INTERVAL);

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        local_log_polls(logs, false, Self::poll)
    }
}

impl ClaudeCcusagePeriods {
    async fn poll(logs: ClaudeLocalLogs) -> Result<Self, Error> {
        Ok(ClaudeCcusagePeriods {
            recent: ccusage::recent(Agent::ClaudeCode).await?,
            budget: logs.budget,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusageBlock {
    pub active: Option<BillingBlock>,
}

impl ReadingSource for ClaudeCcusageBlock {
    type Error = Error;
    type Key = ClaudeLocalLogs;

    const NAME: &'static str = "claude-ccusage-block";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(POLL_INTERVAL);

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        local_log_polls(logs, true, Self::poll)
    }
}

impl ClaudeCcusageBlock {
    async fn poll(_logs: ClaudeLocalLogs) -> Result<Self, Error> {
        Ok(ClaudeCcusageBlock {
            active: ccusage::active_blocks(Agent::ClaudeCode)
                .await?
                .into_iter()
                .next(),
        })
    }
}
