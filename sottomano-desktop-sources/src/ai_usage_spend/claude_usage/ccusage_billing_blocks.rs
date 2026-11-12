use std::{future::ready, path::PathBuf};

use chrono::{DateTime, Local, Utc};
use futures::Stream;

use super::{Error, local_log_readings};
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
    pub credentials: Option<PathBuf>,
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

    const NAME: &'static str = "Claude Code local usage";

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        local_log_readings(logs, SubscriptionCoverage::Uncovered, |logs| {
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
pub struct ClaudeCcusageBlock {
    pub active: Option<BillingBlock>,
}

impl SnapshotSource for ClaudeCcusageBlock {
    type Error = Error;
    type StreamInputs = ClaudeLocalLogs;

    const NAME: &'static str = "Claude Code billing block";

    fn delivery_stream(logs: ClaudeLocalLogs) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        local_log_readings(logs, SubscriptionCoverage::Covered, |_| {
            SnapshotDelivery::live_push_deliveries((), |()| {
                ready(Ok(ccusage_cli_reports::transcript_changes(
                    Agent::ClaudeCode,
                )
                .paced_rereads(
                    TRANSCRIPT_REREAD_MINIMUM_GAP,
                    Self::read,
                    |block| {
                        block
                            .as_ref()
                            .ok()
                            .and_then(|block| block.active.as_ref())
                            .map_or(RereadSchedule::WaitForTrigger, |active| {
                                RereadSchedule::RecheckAt(ccusage_cli_reports::instant_of(
                                    active.ends_at,
                                ))
                            })
                    },
                )))
            })
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
