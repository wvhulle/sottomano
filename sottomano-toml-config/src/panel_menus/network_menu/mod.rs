pub mod reachability;

use serde::{Deserialize, Serialize};

use self::reachability::ReachabilityConfig;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkMenuConfig {
    pub reachability: ReachabilityConfig,
}
