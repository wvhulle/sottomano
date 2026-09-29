use std::time::Duration;

use super::Error;
use crate::data::source::{
    Source,
    ai::ccusage::{self, Agent, POLL_INTERVAL, Usage},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotCcusage {
    pub month: String,
    pub usage: Usage,
}

impl Source for CopilotCcusage {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "copilot-usage-ccusage";
    const INTERVAL: Duration = POLL_INTERVAL;

    async fn available((): &()) -> bool {
        ccusage::installed().await
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        let current = ccusage::this_month(Agent::CopilotCli).await?;
        Ok(CopilotCcusage {
            month: current.name,
            usage: current.usage,
        })
    }
}
