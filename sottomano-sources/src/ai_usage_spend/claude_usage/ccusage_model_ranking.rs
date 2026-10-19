use std::{future::ready, path::PathBuf};

use futures::{Stream, StreamExt, stream};

use super::{Error, LoginKind, logins};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    ai_usage_spend::{
        ccusage_cli_reports::{self, Agent},
        model_spend_ranking::ModelRanking,
    },
    pushed_delivery_stream,
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusageModels {
    pub covered_by_subscription: bool,
    pub today: ModelRanking,
    pub week: ModelRanking,
    pub month: ModelRanking,
}

impl ReadingSource for ClaudeCcusageModels {
    type Error = Error;
    type StreamInputs = Option<PathBuf>;

    const NAME: &'static str = "claude-ccusage-models";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(
        credentials: Option<PathBuf>,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(ccusage_cli_reports::installed()).while_held(
            move || {
                let subscription_logins = match credentials.clone() {
                    Some(credentials) => logins(credentials)
                        .map(|kind| kind == Some(LoginKind::Subscription))
                        .left_stream(),
                    None => stream::iter([false]).right_stream(),
                };
                subscription_logins
                    .distinct_until_changed()
                    .switch_map(|covered_by_subscription| {
                        pushed_delivery_stream(ready(Ok(reread_on_trigger_or_deadline(
                            ccusage_cli_reports::transcript_changes(Agent::ClaudeCode),
                            move || Self::read(covered_by_subscription),
                            |_| ccusage_cli_reports::next_local_midnight(),
                        ))))
                    })
            },
            SourceDelivery::Absent,
        )
    }
}

impl ClaudeCcusageModels {
    async fn read(covered_by_subscription: bool) -> Result<Self, Error> {
        let recent = ccusage_cli_reports::recent(Agent::ClaudeCode).await?;
        Ok(ClaudeCcusageModels {
            covered_by_subscription,
            today: recent.today.usage.models,
            week: recent.week.usage.models,
            month: recent.month.usage.models,
        })
    }
}
