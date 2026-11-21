use sottomano_compositor_model::compositor_window_facts::WorkspaceName;

use crate::desktop_windows_apps::workspace_window_snapshot::WorkspaceIndicator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredWorkspaces {
    workspace_names: Vec<String>,
}

impl FromIterator<String> for ConfiguredWorkspaces {
    fn from_iter<I: IntoIterator<Item = String>>(names: I) -> Self {
        Self {
            workspace_names: names.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMove {
    pub name: String,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfiguredOrderStatus {
    Followed,
    Diverged,
}

impl ConfiguredWorkspaces {
    fn present_on<'s>(&'s self, live: &[WorkspaceIndicator]) -> Vec<&'s str> {
        self.workspace_names
            .iter()
            .map(String::as_str)
            .filter(|name| {
                live.iter().any(|indicator| {
                    matches!(&indicator.name, WorkspaceName::Named(live_name) if live_name == name)
                })
            })
            .collect()
    }

    #[must_use]
    pub fn order_status_on(&self, live: &[WorkspaceIndicator]) -> ConfiguredOrderStatus {
        let configured = self.present_on(live);
        let leading: Vec<&str> = live
            .iter()
            .take(configured.len())
            .filter_map(|indicator| match &indicator.name {
                WorkspaceName::Named(name) => Some(name.as_str()),
                WorkspaceName::Unnamed => None,
            })
            .collect();
        if leading == configured {
            ConfiguredOrderStatus::Followed
        } else {
            ConfiguredOrderStatus::Diverged
        }
    }

    #[must_use]
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
