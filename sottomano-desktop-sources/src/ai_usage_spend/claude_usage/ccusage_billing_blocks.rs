use std::future::ready;

use chrono::{DateTime, Local, Utc};
use futures::Stream;
use sottomano_toml_config::secrets::StoredSecret;

use super::{Error, local_log_samples};
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        ccusage_cli_reports::{
            self, Agent, BillingBlock, RecentUsage, TRANSCRIPT_REREAD_MINIMUM_GAP,
        },
        claude_usage::SubscriptionCoverage,
        usage_quota::Quota,
        working_time_forecast::SpendBudget,
    },
    delivery_cadence_drivers::{ChangeTriggerStreams, RereadSchedule},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusagePeriods {
    pub recent: RecentUsage,
    pub budget: SpendBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaudeLocalLogs {
    pub budget: SpendBudget,
    pub credentials: StoredSecret,
}

impl ClaudeCcusagePeriods {
    #[must_use]
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        vec![self.recent.month.budget_quota(
            "Claude Code monthly budget",
            &self.budget,
            now.with_timezone(&Local),
        )]
    }
}

impl SnapshotSource for ClaudeCcusagePeriods {
    type Error = Error;
    type StreamInputs = ClaudeLocalLogs;

    const NAME: &'static str = "API-priced spend";

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        local_log_samples(logs, SubscriptionCoverage::Uncovered, |logs| {
            SnapshotDelivery::live_push_deliveries(logs, |logs| {
                ready(Ok(ccusage_cli_reports::transcript_changes(
                    Agent::ClaudeCode,
                )
                .paced_rereads(
                    TRANSCRIPT_REREAD_MINIMUM_GAP,
                    move || Self::read(logs.clone()),
                    |_| ccusage_cli_reports::next_local_midnight(),
                )))
            })
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
pub enum ClaudeCcusageBlock {
    Active(BillingBlock),
    NoActiveBlock,
}

impl SnapshotSource for ClaudeCcusageBlock {
    type Error = Error;
    type StreamInputs = ClaudeLocalLogs;

    const NAME: &'static str = "5-hour block spend";

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        local_log_samples(logs, SubscriptionCoverage::Uncovered, |_| {
            SnapshotDelivery::live_push_deliveries((), |()| {
                ready(Ok(ccusage_cli_reports::transcript_changes(
                    Agent::ClaudeCode,
                )
                .paced_rereads(
                    TRANSCRIPT_REREAD_MINIMUM_GAP,
                    Self::read,
                    |block| match block {
                        Ok(ClaudeCcusageBlock::Active(active)) => RereadSchedule::RecheckAt(
                            ccusage_cli_reports::monotonic_instant(active.ends_at),
                        ),
                        Ok(ClaudeCcusageBlock::NoActiveBlock) | Err(_) => {
                            RereadSchedule::WaitForTrigger
                        }
                    },
                )))
            })
        })
    }
}

impl ClaudeCcusageBlock {
    async fn read() -> Result<Self, Error> {
        Ok(ccusage_cli_reports::active_blocks(Agent::ClaudeCode)
            .await?
            .into_iter()
            .next()
            .map_or(
                ClaudeCcusageBlock::NoActiveBlock,
                ClaudeCcusageBlock::Active,
            ))
    }
}
