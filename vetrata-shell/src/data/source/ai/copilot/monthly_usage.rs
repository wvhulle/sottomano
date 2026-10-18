use chrono::{DateTime, Local, Utc};
use iced::futures::{Stream, stream};

use super::Error;
use crate::{
    data::source::{
        DeliveryCadence, ReadingSource, SourceDelivery,
        ai::{
            ccusage::{self, Agent, POLL_INTERVAL, Period},
            quota::Quota,
            working_time_forecast::SpendBudget,
        },
        polled_delivery_stream,
    },
    stream_operators::StreamOperators,
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
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(POLL_INTERVAL);

    fn delivery_stream(budget: SpendBudget) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(ccusage::installed()).while_held(
            move || polled_delivery_stream(budget.clone(), Self::poll),
            SourceDelivery::Absent,
        )
    }
}

impl CopilotCcusage {
    async fn poll(budget: SpendBudget) -> Result<Self, Error> {
        let recent = ccusage::recent(Agent::CopilotCli).await?;
        Ok(CopilotCcusage {
            period: recent.month,
            budget,
        })
    }
}
