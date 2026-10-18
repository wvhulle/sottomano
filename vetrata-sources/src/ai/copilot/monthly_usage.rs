use std::future::ready;

use chrono::{DateTime, Local, Utc};
use futures::{Stream, stream};

use super::Error;
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    ai::{
        ccusage::{self, Agent, Period},
        quota::Quota,
        working_time_forecast::SpendBudget,
    },
    pushed_delivery_stream,
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
        self.period
            .budget_quota(
                "Copilot CLI monthly budget",
                &self.budget,
                now.with_timezone(&Local),
            )
            .into_iter()
            .collect()
    }
}

impl ReadingSource for CopilotCcusage {
    type Error = Error;
    type StreamInputs = SpendBudget;

    const NAME: &'static str = "copilot-usage-ccusage";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(budget: SpendBudget) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(ccusage::installed()).while_held(
            move || {
                let budget = budget.clone();
                pushed_delivery_stream(ready(Ok(reread_on_trigger_or_deadline(
                    ccusage::transcript_changes(Agent::CopilotCli),
                    move || Self::read(budget.clone()),
                    |_| ccusage::next_local_midnight(),
                ))))
            },
            SourceDelivery::Absent,
        )
    }
}

impl CopilotCcusage {
    async fn read(budget: SpendBudget) -> Result<Self, Error> {
        let recent = ccusage::recent(Agent::CopilotCli).await?;
        Ok(CopilotCcusage {
            period: recent.month,
            budget,
        })
    }
}
