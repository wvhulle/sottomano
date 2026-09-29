use std::time::Duration;

use super::Error;
use crate::data::source::{
    Source,
    ai::ccusage::{self, Agent, POLL_INTERVAL, Usage},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusage {
    pub today: Usage,
}

impl Source for ClaudeCcusage {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "claude-usage-ccusage";
    const INTERVAL: Duration = POLL_INTERVAL;

    async fn available((): &()) -> bool {
        ccusage::installed().await
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        Ok(ClaudeCcusage {
            today: ccusage::today(Agent::ClaudeCode).await?,
        })
    }
}
