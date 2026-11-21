use std::{
    env,
    path::{Path, PathBuf},
};

use kdl::{KdlDocument, KdlError};
use tokio::fs;

pub fn parse_niri_kdl(text: &str) -> Result<KdlDocument, KdlError> {
    KdlDocument::parse(text).or_else(|_| KdlDocument::parse_v1(text))
}

fn niri_config_path(config_home: &Path) -> PathBuf {
    env::var_os("NIRI_CONFIG").map_or_else(
        || config_home.join("niri").join("config.kdl"),
        PathBuf::from,
    )
}

fn workspace_names_declared_in(text: &str) -> Result<Vec<String>, KdlError> {
    let document = parse_niri_kdl(text)?;
    Ok(document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "workspace")
        .filter_map(|node| node.entries().first()?.value().as_string())
        .map(str::to_owned)
        .collect())
}

pub fn configured_niri_workspace_names(
    config_home: &Path,
) -> impl Future<Output = Result<Vec<String>, String>> + use<> {
    let path = niri_config_path(config_home);
    async move {
        let text = fs::read_to_string(&path)
            .await
            .map_err(|read_error| format!("reading {} failed: {read_error}", path.display()))?;
        workspace_names_declared_in(&text)
            .map_err(|parse_error| format!("{} is not valid KDL: {parse_error}", path.display()))
    }
}
