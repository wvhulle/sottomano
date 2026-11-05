use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WoodpeckerAgentConfig {
    pub label: String,
    pub varz_url: String,
    pub engine_socket: PathBuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ForgejoRunnerConfig {
    pub label: String,
    pub unit: String,
    pub engine_socket: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CiRunnersConfig {
    pub woodpecker_agents: Vec<WoodpeckerAgentConfig>,
    pub forgejo_runners: Vec<ForgejoRunnerConfig>,
}

impl CiRunnersConfig {
    #[must_use]
    pub fn no_runners_configured(&self) -> bool {
        self.woodpecker_agents.is_empty() && self.forgejo_runners.is_empty()
    }
}
