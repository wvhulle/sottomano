use std::{
    env,
    path::{Path, PathBuf},
};

use iced::Task;
use kdl::KdlDocument;
use tokio::fs;

use crate::data::source::niri::WorkspaceIndicator;

#[derive(Debug, Clone)]
pub struct NiriConfigFile {
    path: PathBuf,
}

impl NiriConfigFile {
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: env::var_os("NIRI_CONFIG").map_or_else(
                || config_home.join("niri").join("config.kdl"),
                PathBuf::from,
            ),
        }
    }

    pub fn read_workspaces(&self) -> Task<Result<ConfiguredWorkspaces, String>> {
        let path = self.path.clone();
        Task::future(async move {
            let text = fs::read_to_string(&path)
                .await
                .map_err(|e| format!("reading {} failed: {e}", path.display()))?;
            ConfiguredWorkspaces::declared_in(&text)
                .map_err(|e| format!("{} is not valid KDL: {e}", path.display()))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredWorkspaces(Vec<String>);

#[cfg(test)]
impl FromIterator<String> for ConfiguredWorkspaces {
    fn from_iter<I: IntoIterator<Item = String>>(names: I) -> Self {
        Self(names.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMove {
    pub name: String,
    pub index: usize,
}

impl ConfiguredWorkspaces {
    fn declared_in(text: &str) -> Result<Self, kdl::KdlError> {
        let document = KdlDocument::parse(text).or_else(|_| KdlDocument::parse_v1(text))?;
        Ok(Self(
            document
                .nodes()
                .iter()
                .filter(|node| node.name().value() == "workspace")
                .filter_map(|node| node.entries().first()?.value().as_string())
                .map(str::to_owned)
                .collect(),
        ))
    }

    fn present_on<'s>(&'s self, live: &[WorkspaceIndicator]) -> Vec<&'s str> {
        self.0
            .iter()
            .map(String::as_str)
            .filter(|name| live.iter().any(|p| p.name.as_deref() == Some(*name)))
            .collect()
    }

    pub fn is_followed_on(&self, live: &[WorkspaceIndicator]) -> bool {
        let configured = self.present_on(live);
        let leading: Vec<&str> = live
            .iter()
            .take(configured.len())
            .filter_map(|p| p.name.as_deref())
            .collect();
        leading == configured
    }

    pub fn restoring_moves(&self, live: &[WorkspaceIndicator]) -> Vec<WorkspaceMove> {
        self.present_on(live)
            .into_iter()
            .zip(1..)
            .map(|(name, index)| WorkspaceMove {
                name: name.to_owned(),
                index,
            })
            .collect()
    }
}
