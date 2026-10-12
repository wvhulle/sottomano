use std::time::Duration;

use chrono::{DateTime, Local, Utc};

use super::Error;
use crate::data::source::{
    Source,
    ai::{
        ccusage::{self, Agent, POLL_INTERVAL, Period, SpendBudget},
        quota::{Quota, QuotaLimited},
    },
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
    const INTERVAL: Duration = POLL_INTERVAL;

    async fn available(_budget: &SpendBudget) -> bool {
        ccusage::installed().await
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
