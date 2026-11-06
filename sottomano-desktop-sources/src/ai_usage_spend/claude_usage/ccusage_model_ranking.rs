use std::{future::ready, path::PathBuf};

use futures::{Stream, StreamExt, stream};

use super::{Error, LoginKind, logins};
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        ccusage_cli_reports::{self, Agent},
        model_spend_ranking::ModelRanking,
    },
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusageModels {
    pub covered_by_subscription: bool,
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
        stream::once(ccusage_cli_reports::installed()).available_while(move || {
            let subscription_logins = match credentials.clone() {
                Some(credentials) => logins(credentials)
                    .map(|kind| kind == Some(LoginKind::Subscription))
                    .left_stream(),
                None => stream::iter([false]).right_stream(),
            };
            subscription_logins
                .distinct_until_changed()
                .switch_map(|covered_by_subscription| {
                    SnapshotDelivery::live_push_deliveries(ready(Ok(
                        reread_on_trigger_or_deadline(
                            ccusage_cli_reports::transcript_changes(Agent::ClaudeCode),
                            move || Self::read(covered_by_subscription),
                            |_| ccusage_cli_reports::next_local_midnight(),
                        ),
                    )))
                })
        })
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
