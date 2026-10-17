use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use iced::futures::{Stream, StreamExt, stream};

use super::Error;
use crate::data::source::{
    Delivery, PolledSource, Source,
    ai::{
        ccusage::{self, Agent, POLL_INTERVAL, Period},
        quota::{Quota, QuotaLimited},
        working_time_forecast::SpendBudget,
    },
    present_or_absent, ticked_polls,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotCcusage {
    pub month: String,
    pub period: Period,
    pub budget: SpendBudget,
}

impl QuotaLimited for CopilotCcusage {
    fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
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

impl Source for CopilotCcusage {
    type Error = Error;
    type Key = SpendBudget;

    const NAME: &'static str = "copilot-usage-ccusage";
}

impl PolledSource for CopilotCcusage {
    const INTERVAL: Duration = POLL_INTERVAL;

    fn polls(budget: SpendBudget) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(stream::once(ccusage::installed()), move || {
            ticked_polls::<Self>(budget.clone()).boxed()
        })
    }

    async fn poll(budget: SpendBudget) -> Result<Self, Self::Error> {
        let current = ccusage::this_month(Agent::CopilotCli).await?;
        Ok(CopilotCcusage {
            month: current.name,
            period: current.period,
            budget,
        })
    }
}
