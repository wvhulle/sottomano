pub mod browser_page;
mod command_icons;

use browser_page::{BrowserBrand, BrowserPage, firefox_favicon_key};
use command_icons::command_icon;

use crate::data::source::kitty_windows::{
    KITTY_APP_ID,
    kitty_window_label::{ForegroundActivity, KittyWindowLabel, WorkingLocation},
};

const FIREFOX_APP_IDS: [&str; 2] = ["firefox", "org.mozilla.firefox"];

const SSH_COMMAND: &str = "ssh";

#[derive(Debug, Clone)]
pub enum IconCandidate {
    Theme(String),
    AppId(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowLabel {
    KittyAgent(KittyWindowLabel),
    Firefox {
        page: BrowserPage,
        favicon_key: Option<String>,
    },
    BrandedBrowser(BrowserPage),
    Titled(String),
}

impl WindowLabel {
    pub fn classify(
        app_id: Option<&str>,
        title: &str,
        kitty_label: Option<KittyWindowLabel>,
    ) -> Self {
        let titled = || WindowLabel::Titled(title.to_owned());
        let Some(app_id) = app_id else {
            return titled();
        };
        if let Some(label) = kitty_label.filter(|_| app_id == KITTY_APP_ID) {
            return WindowLabel::KittyAgent(label);
        }
        if FIREFOX_APP_IDS.contains(&app_id) {
            let Some(page) = BrowserPage::in_firefox(title) else {
                return titled();
            };
            return WindowLabel::Firefox {
                favicon_key: firefox_favicon_key(title),
                page,
            };
        }
        let Ok(brand) = BrowserBrand::try_from(app_id) else {
            return titled();
        };
        BrowserPage::in_branded_browser(title, brand)
            .map_or_else(titled, WindowLabel::BrandedBrowser)
    }

    pub fn theme_icon(&self) -> Option<String> {
        let WindowLabel::KittyAgent(label) = self else {
            return None;
        };
        match &label.working_location {
            WorkingLocation::RemoteHost(_) => command_icon(SSH_COMMAND),
            WorkingLocation::LocalDirectory(_) => {
                command_icon(label.foreground_activity.as_ref()?.command_name()?)
            }
        }
    }

    pub fn favicon_key(&self) -> Option<&str> {
        let WindowLabel::Firefox { favicon_key, .. } = self else {
            return None;
        };
        favicon_key.as_deref()
    }

    pub fn shown_texts(&self) -> Vec<&str> {
        match self {
            WindowLabel::KittyAgent(label) => [Some(label.working_location.display_text())]
                .into_iter()
                .chain([label
                    .foreground_activity
                    .as_ref()
                    .map(ForegroundActivity::display_text)])
                .flatten()
                .collect(),
            WindowLabel::Firefox { page, .. } | WindowLabel::BrandedBrowser(page) => {
                [Some(page.page.as_str()), page.site.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect()
            }
            WindowLabel::Titled(title) => vec![title],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kitty_running(command_line: &str) -> KittyWindowLabel {
        KittyWindowLabel {
            working_location: WorkingLocation::LocalDirectory("vetrata".to_owned()),
            foreground_activity: Some(ForegroundActivity::ShellCommand {
                command_line: command_line.to_owned(),
                program_name: None,
            }),
        }
    }

    fn kitty_theme_icon(label: KittyWindowLabel) -> Option<String> {
        WindowLabel::KittyAgent(label).theme_icon()
    }

    #[test]
    fn firefox_windows_carry_their_page_and_favicon_key() {
        assert_eq!(
            WindowLabel::classify(
                Some("firefox"),
                "Inbox — user@example.com — Mozilla Firefox",
                None
            ),
            WindowLabel::Firefox {
                page: BrowserPage {
                    page: "Inbox".to_owned(),
                    site: Some("user@example.com".to_owned()),
                },
                favicon_key: Some("Inbox — user@example.com".to_owned()),
            }
        );
        assert_eq!(
            WindowLabel::classify(Some("firefox"), "about:blank", None),
            WindowLabel::Titled("about:blank".to_owned())
        );
    }

    #[test]
    fn branded_browsers_split_and_unknown_app_ids_stay_whole() {
        assert_eq!(
            WindowLabel::classify(
                Some("org.chromium.Chromium"),
                "Odin-rs · GitHub - Chromium",
                None
            ),
            WindowLabel::BrandedBrowser(BrowserPage {
                page: "Odin-rs".to_owned(),
                site: Some("GitHub".to_owned()),
            })
        );
        assert_eq!(
            WindowLabel::classify(Some("chromium-like"), "Odin-rs - Chromium", None),
            WindowLabel::Titled("Odin-rs - Chromium".to_owned())
        );
    }

    #[test]
    fn kitty_windows_use_the_agent_label_only_when_one_was_supplied() {
        assert_eq!(
            WindowLabel::classify(Some("kitty"), "raw title", Some(kitty_running("cargo"))),
            WindowLabel::KittyAgent(kitty_running("cargo"))
        );
        assert_eq!(
            WindowLabel::classify(Some("kitty"), "vetrata — cargo", None),
            WindowLabel::Titled("vetrata — cargo".to_owned())
        );
    }

    #[test]
    fn kitty_theme_icon_matches_the_running_command() {
        assert_eq!(
            kitty_theme_icon(kitty_running("nixos-rebuild")),
            Some("nix-snowflake".to_owned())
        );
        assert_eq!(kitty_theme_icon(kitty_running("unknown-tool")), None);
        assert_eq!(
            kitty_theme_icon(kitty_running("./scripts/check.nu")),
            Some("system-run".to_owned())
        );
        assert_eq!(
            kitty_theme_icon(KittyWindowLabel {
                working_location: WorkingLocation::RemoteHost("builder".to_owned()),
                foreground_activity: None,
            }),
            Some("gnome-fs-ssh".to_owned())
        );
    }
}
