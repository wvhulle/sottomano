use std::{future::ready, path::PathBuf};

use futures::{Stream, StreamExt, stream};
use futures_rx::RxExt;

use super::{Error, logins};
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        ccusage_cli_reports::{self, Agent, TRANSCRIPT_REREAD_MINIMUM_GAP},
        claude_usage::SubscriptionCoverage,
        model_spend_ranking::ModelRanking,
    },
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::ChangeTriggerStreams,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusageModels {
    pub subscription_coverage: SubscriptionCoverage,
    pub today: ModelRanking,
    pub week: ModelRanking,
    pub month: ModelRanking,
}

impl SnapshotSource for ClaudeCcusageModels {
    type Error = Error;
    type StreamInputs = Option<PathBuf>;

    const NAME: &'static str = "Claude Code models";

    fn delivery_stream(
        credentials: Option<PathBuf>,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(ccusage_cli_reports::ccusage_availability()).while_available(
            move || {
                let subscription_logins = match credentials.clone() {
                    Some(credentials) => logins(credentials)
                        .map(SubscriptionCoverage::from)
                        .left_stream(),
                    None => stream::iter([SubscriptionCoverage::Uncovered]).right_stream(),
                };
                subscription_logins
                    .distinct_until_changed()
                    .switch_map(|subscription_coverage| {
                        SnapshotDelivery::live_push_deliveries(
                            subscription_coverage,
                            |subscription_coverage| {
                                ready(Ok(ccusage_cli_reports::transcript_changes(
                                    Agent::ClaudeCode,
                                )
                                .paced_rereads(
                                    TRANSCRIPT_REREAD_MINIMUM_GAP,
                                    move || Self::read(subscription_coverage),
                                    |_| ccusage_cli_reports::next_local_midnight(),
                                )))
                            },
                        )
                    })
            },
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}

impl ClaudeCcusageModels {
    async fn read(subscription_coverage: SubscriptionCoverage) -> Result<Self, Error> {
        let recent = ccusage_cli_reports::recent(Agent::ClaudeCode).await?;
        Ok(ClaudeCcusageModels {
            subscription_coverage,
            today: recent.today.usage.models,
            week: recent.week.usage.models,
            month: recent.month.usage.models,
        })
    }
}
