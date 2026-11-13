use std::{
    borrow::Cow,
    ffi::OsString,
    fmt,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    str::FromStr,
    string::FromUtf8Error,
};

use chrono::TimeDelta;
use sottomano_toml_config::widget_groups::session_focus::SessionFocusConfig;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FocusUnit {
    App(String),
    Project(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFacet {
    pub facet: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoredFacetError {
    #[error("unknown focus facet {0}")]
    UnknownFacet(String),
    #[error("stored app id is not unicode: {0}")]
    NonUnicodeAppId(#[from] FromUtf8Error),
}

impl From<&FocusUnit> for StoredFacet {
    fn from(unit: &FocusUnit) -> Self {
        match unit {
            FocusUnit::App(app_id) => StoredFacet {
                facet: "app".to_owned(),
                value: app_id.as_bytes().to_vec(),
            },
            FocusUnit::Project(root) => StoredFacet {
                facet: "project".to_owned(),
                value: root.as_os_str().as_bytes().to_vec(),
            },
        }
    }
}

impl TryFrom<StoredFacet> for FocusUnit {
    type Error = StoredFacetError;

    fn try_from(stored: StoredFacet) -> Result<Self, StoredFacetError> {
        match stored.facet.as_str() {
            "app" => Ok(FocusUnit::App(String::from_utf8(stored.value)?)),
            "project" => Ok(FocusUnit::Project(PathBuf::from(OsString::from_vec(
                stored.value,
            )))),
            _ => Err(StoredFacetError::UnknownFacet(stored.facet)),
        }
    }
}

impl FocusUnit {
    #[must_use]
    pub fn label(&self) -> Cow<'_, str> {
        match self {
            FocusUnit::App(app_id) => Cow::Borrowed(app_id),
            FocusUnit::Project(root) => Cow::Owned(
                root.file_name()
                    .map_or_else(|| root.display(), |name| Path::new(name).display())
                    .to_string(),
            ),
        }
    }

    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            FocusUnit::App(_) => "app",
            FocusUnit::Project(_) => "project",
        }
    }

    #[must_use]
    pub fn budget(&self, config: &SessionFocusConfig) -> Option<TimeDelta> {
        match self {
            FocusUnit::App(app_id) => config.app_daily_budget_minutes.get(app_id),
            FocusUnit::Project(root) => root
                .to_str()
                .and_then(|root_text| config.project_daily_budget_minutes.get(root_text)),
        }
        .map(|minutes| TimeDelta::minutes(i64::from(*minutes)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Active,
    Idle,
    Asleep,
    ScreenOff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPresence(pub String);

impl fmt::Display for UnknownPresence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown presence state {}", self.0)
    }
}

impl FromStr for Presence {
    type Err = UnknownPresence;

    fn from_str(stored: &str) -> Result<Self, UnknownPresence> {
        match stored {
            "active" => Ok(Presence::Active),
            "idle" => Ok(Presence::Idle),
            "asleep" => Ok(Presence::Asleep),
            "screen_off" => Ok(Presence::ScreenOff),
            _ => Err(UnknownPresence(stored.to_owned())),
        }
    }
}

impl Presence {
    #[must_use]
    pub const fn stored_name(self) -> &'static str {
        match self {
            Presence::Active => "active",
            Presence::Idle => "idle",
            Presence::Asleep => "asleep",
            Presence::ScreenOff => "screen_off",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_facets_round_trip() {
        let unit = FocusUnit::Project(PathBuf::from("/code/sottomano"));
        assert_eq!(FocusUnit::try_from(StoredFacet::from(&unit)), Ok(unit));
        assert!(
            FocusUnit::try_from(StoredFacet {
                facet: "branch".to_owned(),
                value: b"main".to_vec()
            })
            .is_err()
        );
        assert_eq!("asleep".parse(), Ok(Presence::Asleep));
    }

    #[test]
    fn non_unicode_project_roots_round_trip() {
        let unit = FocusUnit::Project(PathBuf::from(OsString::from_vec(b"/code/caf\xe9".to_vec())));
        assert_eq!(FocusUnit::try_from(StoredFacet::from(&unit)), Ok(unit));
    }

    #[test]
    fn non_unicode_app_ids_are_rejected() {
        assert!(matches!(
            FocusUnit::try_from(StoredFacet {
                facet: "app".to_owned(),
                value: b"firefox\xff".to_vec()
            }),
            Err(StoredFacetError::NonUnicodeAppId(_))
        ));
    }
}
