use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::widget_groups::ElementVisibility;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickAction {
    #[default]
    ActivateItem,
    SecondaryActivateItem,
    FocusAppWindow,
    Ignore,
}

impl ClickAction {
    #[must_use]
    pub const fn press_hint(self) -> &'static str {
        match self {
            ClickAction::ActivateItem => "activate",
            ClickAction::SecondaryActivateItem => "secondary action",
            ClickAction::FocusAppWindow => "focus its window",
            ClickAction::Ignore => "nothing",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrayRule {
    pub icon: ElementVisibility,
    pub left_click: ClickAction,
    pub middle_click: ClickAction,
    pub right_click: ClickAction,
    pub window_app_id: Option<String>,
}

impl Default for TrayRule {
    fn default() -> Self {
        Self {
            icon: ElementVisibility::Shown,
            left_click: ClickAction::ActivateItem,
            middle_click: ClickAction::SecondaryActivateItem,
            right_click: ClickAction::Ignore,
            window_app_id: None,
        }
    }
}

impl TrayRule {
    fn clicks(&self) -> [ClickAction; 3] {
        [self.left_click, self.middle_click, self.right_click]
    }

    fn focusing(app_id: &str) -> Self {
        Self {
            left_click: ClickAction::FocusAppWindow,
            window_app_id: Some(String::from(app_id)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(try_from = "TrayItems")]
pub struct TrayConfig {
    items: BTreeMap<String, TrayRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct TrayItems {
    items: BTreeMap<String, TrayRule>,
}

impl Default for TrayItems {
    fn default() -> Self {
        Self {
            items: [
                ("Signal_status_icon_1", TrayRule::focusing("signal")),
                ("spotify-client", TrayRule::focusing("spotify")),
            ]
            .into_iter()
            .map(|(id, rule)| (String::from(id), rule))
            .collect(),
        }
    }
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            items: TrayItems::default().items,
        }
    }
}

impl TryFrom<TrayItems> for TrayConfig {
    type Error = String;

    fn try_from(raw: TrayItems) -> Result<Self, Self::Error> {
        for (id, rule) in &raw.items {
            if id.is_empty() {
                return Err(String::from("tray item id is empty"));
            }
            match (
                &rule.window_app_id,
                rule.clicks().contains(&ClickAction::FocusAppWindow),
            ) {
                (Some(app_id), _) if app_id.is_empty() => {
                    return Err(format!("tray item {id} has an empty window_app_id"));
                }
                (None, true) => {
                    return Err(format!(
                        "tray item {id} clicks focus_app_window but names no window_app_id"
                    ));
                }
                (Some(app_id), false) => {
                    return Err(format!(
                        "tray item {id} names window_app_id {app_id} but no click focuses a window"
                    ));
                }
                _ => {}
            }
        }
        Ok(Self { items: raw.items })
    }
}

impl TrayConfig {
    #[must_use]
    pub fn rule(&self, id: &str) -> TrayRule {
        self.items.get(id).cloned().unwrap_or_default()
    }
}
