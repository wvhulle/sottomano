use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Local, Utc};
use iced::futures::Stream;

use super::{Error, local_log_polls};
use crate::data::source::{
    Delivery, PolledSource, Source,
    ai::{
        ccusage::{self, Agent, BillingBlock, POLL_INTERVAL, RecentUsage},
        quota::{Quota, QuotaLimited},
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

impl QuotaLimited for ClaudeCcusagePeriods {
    fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
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

impl Source for ClaudeCcusagePeriods {
    type Error = Error;
    type Key = ClaudeLocalLogs;

    const NAME: &'static str = "claude-ccusage-periods";
}

impl PolledSource for ClaudeCcusagePeriods {
    const INTERVAL: Duration = POLL_INTERVAL;

    fn polls(logs: ClaudeLocalLogs) -> impl Stream<Item = Delivery<Self>> + Send {
        local_log_polls::<Self>(logs, false)
    }

    async fn poll(logs: ClaudeLocalLogs) -> Result<Self, Self::Error> {
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

impl Source for ClaudeCcusageBlock {
    type Error = Error;
    type Key = ClaudeLocalLogs;

    const NAME: &'static str = "claude-ccusage-block";
}

impl PolledSource for ClaudeCcusageBlock {
    const INTERVAL: Duration = POLL_INTERVAL;

    fn polls(logs: ClaudeLocalLogs) -> impl Stream<Item = Delivery<Self>> + Send {
        local_log_polls::<Self>(logs, true)
    }

    async fn poll(_logs: ClaudeLocalLogs) -> Result<Self, Self::Error> {
        Ok(ClaudeCcusageBlock {
            active: ccusage::active_blocks(Agent::ClaudeCode)
                .await?
                .into_iter()
                .next(),
        })
    }
}
