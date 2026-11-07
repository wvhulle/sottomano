use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::widget_groups::ElementVisibility;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickAction {
    ActivateItem,
    SecondaryActivateItem,
    #[default]
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
            left_click: ClickAction::FocusAppWindow,
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
                ("steam", TrayRule::focusing("steam")),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed_tray_config(toml_text: &str) -> Result<TrayConfig, String> {
        toml::from_str::<TrayConfig>(toml_text).map_err(|parse_failure| parse_failure.to_string())
    }

    #[test]
    fn unconfigured_item_focuses_its_window_on_left_click() {
        let unconfigured_rule = TrayConfig::default().rule("page.codeberg.wvhulle.Bindrune");
        assert_eq!(unconfigured_rule.left_click, ClickAction::FocusAppWindow);
        assert_eq!(unconfigured_rule.window_app_id, None);
    }

    #[test]
    fn steam_window_is_found_by_its_app_id() {
        assert_eq!(
            TrayConfig::default().rule("steam").window_app_id.as_deref(),
            Some("steam")
        );
    }

    #[test]
    fn focus_without_window_app_id_matches_by_process() {
        let tray_config = parsed_tray_config("[items.chat]\nleft_click = \"focus_app_window\"\n")
            .expect("focusing without an app id is valid");
        assert_eq!(tray_config.rule("chat").window_app_id, None);
    }

    #[test]
    fn window_app_id_without_focusing_click_is_rejected() {
        let rejection = parsed_tray_config(
            "[items.chat]\nleft_click = \"activate_item\"\nwindow_app_id = \"chat\"\n",
        )
        .expect_err("an unused window_app_id is rejected");
        assert!(
            rejection.contains("no click focuses a window"),
            "{rejection}"
        );
    }

    #[test]
    fn empty_window_app_id_is_rejected() {
        let rejection = parsed_tray_config("[items.chat]\nwindow_app_id = \"\"\n")
            .expect_err("an empty window_app_id is rejected");
        assert!(rejection.contains("empty window_app_id"), "{rejection}");
    }
}
