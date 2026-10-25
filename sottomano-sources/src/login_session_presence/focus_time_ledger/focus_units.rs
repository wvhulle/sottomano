use std::{borrow::Cow, fmt, path::PathBuf, str::FromStr};

use chrono::TimeDelta;
use sottomano_config::widget_groups::focus_time::FocusTimeConfig;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FocusUnit {
    App(String),
    Project(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFacet {
    pub facet: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFacet(pub String);

impl fmt::Display for UnknownFacet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown focus facet {}", self.0)
    }
}

impl From<&FocusUnit> for StoredFacet {
    fn from(unit: &FocusUnit) -> Self {
        match unit {
            FocusUnit::App(app_id) => StoredFacet {
                facet: "app".to_owned(),
                value: app_id.clone(),
            },
            FocusUnit::Project(root) => StoredFacet {
                facet: "project".to_owned(),
                value: root.to_string_lossy().into_owned(),
            },
        }
    }
}

impl TryFrom<StoredFacet> for FocusUnit {
    type Error = UnknownFacet;

    fn try_from(stored: StoredFacet) -> Result<Self, UnknownFacet> {
        match stored.facet.as_str() {
            "app" => Ok(FocusUnit::App(stored.value)),
            "project" => Ok(FocusUnit::Project(PathBuf::from(stored.value))),
            _ => Err(UnknownFacet(stored.facet)),
        }
    }
}

impl FocusUnit {
    #[must_use]
    pub fn label(&self) -> Cow<'_, str> {
        match self {
            FocusUnit::App(app_id) => Cow::Borrowed(app_id),
            FocusUnit::Project(root) => root
                .file_name()
                .map_or_else(|| root.to_string_lossy(), |name| name.to_string_lossy()),
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
    pub fn budget(&self, config: &FocusTimeConfig) -> Option<TimeDelta> {
        match self {
            FocusUnit::App(app_id) => config.app_daily_budget_minutes.get(app_id),
            FocusUnit::Project(root) => config
                .project_daily_budget_minutes
                .get(root.to_string_lossy().as_ref()),
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
                value: "main".to_owned()
            })
            .is_err()
        );
        assert_eq!("asleep".parse(), Ok(Presence::Asleep));
    }
}
