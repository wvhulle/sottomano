use std::time::Duration;

use chrono::{DateTime, Local, Utc};

use super::Error;
use crate::data::source::{
    Source,
    ai::{
        ccusage::{self, Agent, BillingBlock, POLL_INTERVAL, RecentUsage, SpendBudget},
        quota::{Quota, QuotaLimited},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusagePeriods {
    pub recent: RecentUsage,
    pub budget: SpendBudget,
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
    type Key = SpendBudget;

    const NAME: &'static str = "claude-ccusage-periods";
    const INTERVAL: Duration = POLL_INTERVAL;

    async fn available(_budget: &SpendBudget) -> bool {
        ccusage::installed().await
    }

    async fn poll(budget: SpendBudget) -> Result<Self, Self::Error> {
        Ok(ClaudeCcusagePeriods {
            recent: ccusage::recent(Agent::ClaudeCode).await?,
            budget,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusageBlock {
    pub active: Option<BillingBlock>,
}

impl Source for ClaudeCcusageBlock {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "claude-ccusage-block";
    const INTERVAL: Duration = POLL_INTERVAL;

    async fn available((): &()) -> bool {
        ccusage::installed().await
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        Ok(ClaudeCcusageBlock {
            active: ccusage::active_blocks(Agent::ClaudeCode)
                .await?
                .into_iter()
                .next(),
        })
    }
}
