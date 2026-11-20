use std::{collections::HashMap, path::Path};

use serde::Deserialize;
use tokio::fs::read_to_string;

use crate::system_protocol_clients::shell_command_output;

const EVALUATED_VERSION: &str = "p: { storePath = p.outPath; inherit (p) version; }";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlakePin {
    Unreadable,
    MatchesGeneration,
    Moved { version: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatedVersion {
    pub store_path: String,
    pub version: String,
}

#[derive(Deserialize)]
struct FlakeLock {
    root: String,
    nodes: HashMap<String, FlakeLockNode>,
}

#[derive(Deserialize)]
struct FlakeLockNode {
    #[serde(default)]
    inputs: HashMap<String, serde_json::Value>,
    #[serde(default)]
    locked: HashMap<String, serde_json::Value>,
}

pub async fn evaluate_version(installable: &str) -> Option<EvaluatedVersion> {
    let raw = shell_command_output::output(
        "nix",
        &["eval", "--json", installable, "--apply", EVALUATED_VERSION],
    )
    .await
    .inspect_err(|command_failure| {
        tracing::warn!("nix facts: eval {installable}: {command_failure}");
    })
    .ok()?;
    serde_json::from_str(&raw)
        .inspect_err(|parse_failure| {
            tracing::warn!("nix facts: version of {installable}: {parse_failure}");
        })
        .ok()
}

pub async fn flake_pin(
    package_name: &str,
    nixos_flake: &Path,
    generation_revision: &str,
    generation_version: &str,
) -> FlakePin {
    let lock_path = nixos_flake.join("flake.lock");
    let raw_lock = match read_to_string(&lock_path).await {
        Ok(raw_lock) => raw_lock,
        Err(read_failure) => {
            tracing::warn!("nix facts: read {}: {read_failure}", lock_path.display());
            return FlakePin::Unreadable;
        }
    };
    let Some(locked_revision) = locked_nixpkgs_revision(&raw_lock) else {
        tracing::warn!("nix facts: no nixpkgs revision in {}", lock_path.display());
        return FlakePin::Unreadable;
    };
    if locked_revision == generation_revision {
        return FlakePin::MatchesGeneration;
    }
    let installable = format!("github:NixOS/nixpkgs/{locked_revision}#{package_name}");
    let Some(evaluated) = evaluate_version(&installable).await else {
        return FlakePin::Unreadable;
    };
    if evaluated.version == generation_version {
        FlakePin::MatchesGeneration
    } else {
        FlakePin::Moved {
            version: evaluated.version,
        }
    }
}

fn locked_nixpkgs_revision(raw_lock: &str) -> Option<String> {
    let lock: FlakeLock = serde_json::from_str(raw_lock)
        .inspect_err(|parse_failure| {
            tracing::warn!("nix facts: flake.lock: {parse_failure}");
        })
        .ok()?;
    let root_node = lock.nodes.get(&lock.root)?;
    let nixpkgs_node = root_node.inputs.get("nixpkgs")?.as_str()?;
    lock.nodes
        .get(nixpkgs_node)?
        .locked
        .get("rev")?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_nixpkgs_input_resolves_through_its_node_name() {
        let raw_lock = r#"{
            "nodes": {
                "nixpkgs_30": {"locked": {"owner": "NixOS", "repo": "nixpkgs", "rev": "e21fec321d73", "type": "github"}},
                "home-manager": {"inputs": {"nixpkgs": ["nixpkgs"]}, "locked": {"rev": "aaaa"}},
                "root": {"inputs": {"home-manager": "home-manager", "nixpkgs": "nixpkgs_30"}}
            },
            "root": "root",
            "version": 7
        }"#;
        assert_eq!(
            locked_nixpkgs_revision(raw_lock),
            Some("e21fec321d73".to_owned())
        );
    }

    #[test]
    fn missing_nixpkgs_input_yields_no_revision() {
        let raw_lock = r#"{"nodes": {"root": {"inputs": {}}}, "root": "root", "version": 7}"#;
        assert_eq!(locked_nixpkgs_revision(raw_lock), None);
    }
}
