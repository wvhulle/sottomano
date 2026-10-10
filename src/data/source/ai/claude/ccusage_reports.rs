use std::time::Duration;

use chrono::Weekday;

use super::Error;
use crate::data::source::{
    Source,
    ai::ccusage::{self, Agent, BillingBlock, POLL_INTERVAL, RecentUsage},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusagePeriods {
    pub recent: RecentUsage,
    pub working_days: Vec<Weekday>,
}

impl Source for ClaudeCcusagePeriods {
    type Error = Error;
    type Key = Vec<Weekday>;

    const NAME: &'static str = "claude-ccusage-periods";
    const INTERVAL: Duration = POLL_INTERVAL;

    async fn available(_working_days: &Vec<Weekday>) -> bool {
        ccusage::installed().await
    }

    async fn poll(working_days: Vec<Weekday>) -> Result<Self, Self::Error> {
        Ok(ClaudeCcusagePeriods {
            recent: ccusage::recent(Agent::ClaudeCode).await?,
            working_days,
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
