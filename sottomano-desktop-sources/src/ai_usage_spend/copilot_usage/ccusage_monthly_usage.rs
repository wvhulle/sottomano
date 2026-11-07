use std::future::ready;

use chrono::{DateTime, Local, Utc};
use futures::{Stream, stream};

use super::Error;
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        ccusage_cli_reports::{self, Agent, Period},
        usage_quota::Quota,
        working_time_forecast::SpendBudget,
    },
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotCcusage {
    pub period: Period,
    pub budget: SpendBudget,
}

impl CopilotCcusage {
    #[must_use]
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        vec![self.period.budget_quota(
            "Copilot CLI monthly budget",
            &self.budget,
            now.with_timezone(&Local),
        )]
    }
}

impl SnapshotSource for CopilotCcusage {
    type Error = Error;
    type StreamInputs = SpendBudget;

    const NAME: &'static str = "Copilot local usage";

    fn delivery_stream(budget: SpendBudget) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(ccusage_cli_reports::ccusage_availability()).available_while(move || {
            let budget = budget.clone();
            SnapshotDelivery::live_push_deliveries(ready(Ok(reread_on_trigger_or_deadline(
                ccusage_cli_reports::transcript_changes(Agent::CopilotCli),
                move || Self::read(budget.clone()),
                |_| ccusage_cli_reports::next_local_midnight(),
            ))))
        })
    }
}

impl CopilotCcusage {
    async fn read(budget: SpendBudget) -> Result<Self, Error> {
        let recent = ccusage_cli_reports::recent(Agent::CopilotCli).await?;
        Ok(CopilotCcusage {
            period: recent.month,
            budget,
        })
    }
}
