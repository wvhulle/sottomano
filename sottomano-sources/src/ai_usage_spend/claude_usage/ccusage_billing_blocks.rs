use std::{future::ready, path::PathBuf};

use chrono::{DateTime, Local, Utc};
use futures::Stream;

use super::{Error, local_log_readings};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    ai_usage_spend::{
        ccusage_cli_reports::{self, Agent, BillingBlock, RecentUsage},
        usage_quota::Quota,
        working_time_forecast::SpendBudget,
    },
    pushed_delivery_stream,
    stream_operators::reread_on_trigger_or_deadline,
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
    #[must_use]
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
    type StreamInputs = ClaudeLocalLogs;

    const NAME: &'static str = "claude-ccusage-periods";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        local_log_readings(logs, false, |logs| {
            pushed_delivery_stream(ready(Ok(reread_on_trigger_or_deadline(
                ccusage_cli_reports::transcript_changes(Agent::ClaudeCode),
                move || Self::read(logs.clone()),
                |_| ccusage_cli_reports::next_local_midnight(),
            ))))
        })
    }
}

impl ClaudeCcusagePeriods {
    async fn read(logs: ClaudeLocalLogs) -> Result<Self, Error> {
        Ok(ClaudeCcusagePeriods {
            recent: ccusage_cli_reports::recent(Agent::ClaudeCode).await?,
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
    type StreamInputs = ClaudeLocalLogs;

    const NAME: &'static str = "claude-ccusage-block";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        local_log_readings(logs, true, |_| {
            pushed_delivery_stream(ready(Ok(reread_on_trigger_or_deadline(
                ccusage_cli_reports::transcript_changes(Agent::ClaudeCode),
                Self::read,
                |block| {
                    block
                        .as_ref()
                        .ok()
                        .and_then(|block| block.active.as_ref())
                        .map(|active| ccusage_cli_reports::instant_of(active.ends_at))
                },
            ))))
        })
    }
}

impl ClaudeCcusageBlock {
    async fn read() -> Result<Self, Error> {
        Ok(ClaudeCcusageBlock {
            active: ccusage_cli_reports::active_blocks(Agent::ClaudeCode)
                .await?
                .into_iter()
                .next(),
        })
    }
}
