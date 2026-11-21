use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io,
};

use serde::Deserialize;

use crate::hyprland_socket_connection::hyprland_request;

const ADDRESSED_WORKSPACE_KEY_BIT: u64 = 1 << 63;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum HyprlandWorkspaceKind {
    Normal,
    Special,
    #[default]
    #[serde(other)]
    Unstated,
}

#[derive(Debug, Deserialize)]
pub struct HyprlandWorkspaceReference {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    address: Option<String>,
    #[serde(rename = "type", default)]
    kind: HyprlandWorkspaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegularHyprlandWorkspace {
    Numbered { id: u64 },
    Addressed { address: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyprlandWorkspaceIdentity {
    Regular(RegularHyprlandWorkspace),
    Special,
    Unassigned,
}

impl From<&HyprlandWorkspaceReference> for HyprlandWorkspaceIdentity {
    fn from(reference: &HyprlandWorkspaceReference) -> Self {
        if reference.kind == HyprlandWorkspaceKind::Special {
            return HyprlandWorkspaceIdentity::Special;
        }
        if let Some(id) = reference.id {
            return match u64::try_from(id) {
                Ok(positive_id) if positive_id > 0 => {
                    HyprlandWorkspaceIdentity::Regular(RegularHyprlandWorkspace::Numbered {
                        id: positive_id,
                    })
                }
                Ok(_) | Err(_) => HyprlandWorkspaceIdentity::Special,
            };
        }
        match reference.address.as_deref() {
            Some(address) if !address.is_empty() => {
                HyprlandWorkspaceIdentity::Regular(RegularHyprlandWorkspace::Addressed {
                    address: address.to_owned(),
                })
            }
            Some(_) | None => HyprlandWorkspaceIdentity::Unassigned,
        }
    }
}

fn hashed_workspace_address(address: &str) -> u64 {
    let mut address_hasher = DefaultHasher::new();
    address.hash(&mut address_hasher);
    address_hasher.finish()
}

impl RegularHyprlandWorkspace {
    #[must_use]
    pub fn workspace_key(&self) -> u64 {
        match self {
            RegularHyprlandWorkspace::Numbered { id } => *id,
            RegularHyprlandWorkspace::Addressed { address } => {
                ADDRESSED_WORKSPACE_KEY_BIT | hashed_workspace_address(address)
            }
        }
    }
}

pub async fn hyprland_workspace_with_key(
    workspace_key: u64,
) -> io::Result<Option<RegularHyprlandWorkspace>> {
    let listed: Vec<HyprlandWorkspaceReference> =
        serde_json::from_str(&hyprland_request("j/workspaces").await?)?;
    Ok(listed
        .iter()
        .map(HyprlandWorkspaceIdentity::from)
        .find_map(|identity| match identity {
            HyprlandWorkspaceIdentity::Regular(regular)
                if regular.workspace_key() == workspace_key =>
            {
                Some(regular)
            }
            HyprlandWorkspaceIdentity::Regular(_)
            | HyprlandWorkspaceIdentity::Special
            | HyprlandWorkspaceIdentity::Unassigned => None,
        }))
}

#[cfg(test)]
mod tests {
    use super::{
        ADDRESSED_WORKSPACE_KEY_BIT, HyprlandWorkspaceIdentity, HyprlandWorkspaceReference,
        RegularHyprlandWorkspace,
    };

    fn parsed_identity(reference_json: &str) -> HyprlandWorkspaceIdentity {
        let reference: HyprlandWorkspaceReference = serde_json::from_str(reference_json).unwrap();
        HyprlandWorkspaceIdentity::from(&reference)
    }

    #[test]
    fn numbered_workspaces_keep_their_id_as_key() {
        let identity =
            parsed_identity(r#"{"address": "3", "id": 3, "type": "normal", "name": "3"}"#);

        assert_eq!(
            identity,
            HyprlandWorkspaceIdentity::Regular(RegularHyprlandWorkspace::Numbered { id: 3 })
        );
    }

    #[test]
    fn workspaces_without_an_id_are_keyed_by_their_address() {
        let HyprlandWorkspaceIdentity::Regular(games) =
            parsed_identity(r#"{"address": "Games", "type": "normal", "name": "Games"}"#)
        else {
            panic!("a named normal workspace is regular");
        };

        assert_eq!(
            games,
            RegularHyprlandWorkspace::Addressed {
                address: "Games".to_owned()
            }
        );
        assert_ne!(games.workspace_key() & ADDRESSED_WORKSPACE_KEY_BIT, 0);
        assert_ne!(
            games.workspace_key(),
            RegularHyprlandWorkspace::Addressed {
                address: "Movies".to_owned()
            }
            .workspace_key()
        );
    }

    #[test]
    fn special_and_missing_workspaces_are_not_regular() {
        assert_eq!(
            parsed_identity(
                r#"{"address": "special:scratch", "type": "special", "name": "scratch"}"#
            ),
            HyprlandWorkspaceIdentity::Special
        );
        assert_eq!(
            parsed_identity(r#"{"id": -98, "name": "special:scratch"}"#),
            HyprlandWorkspaceIdentity::Special
        );
        assert_eq!(
            parsed_identity(r#"{"address": "", "type": "", "name": ""}"#),
            HyprlandWorkspaceIdentity::Unassigned
        );
    }
}
