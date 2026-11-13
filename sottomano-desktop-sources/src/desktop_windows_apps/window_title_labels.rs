pub mod branded_browser_titles;
mod command_icon_names;
pub mod firefox_page_titles;
pub mod recognised_app_titles;
pub mod split_app_title;
mod thunderbird_mail_titles;
mod visual_studio_code_titles;
mod zed_editor_titles;

use command_icon_names::command_icon;
use firefox_page_titles::firefox_favicon_key;
use recognised_app_titles::{RecognisedApp, RecognisedAppKind};
use split_app_title::SplitAppTitle;

use crate::desktop_windows_apps::terminal_emulator_windows::terminal_shell_label::{
    TerminalShellLabel, WorkingLocation,
};

const SSH_COMMAND: &str = "ssh";

#[derive(Debug, Clone)]
pub enum IconCandidate {
    Theme(String),
    AppId(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowGroupKey<'a> {
    WorkingDirectory(&'a str),
    RemoteHost(&'a str),
    WebSite(&'a str),
    EditorProject(&'a str),
    MailAccount(&'a str),
    Application(&'a str),
}

impl<'a> WindowGroupKey<'a> {
    #[must_use]
    pub fn heading_text(self) -> &'a str {
        match self {
            WindowGroupKey::WorkingDirectory(text)
            | WindowGroupKey::RemoteHost(text)
            | WindowGroupKey::WebSite(text)
            | WindowGroupKey::EditorProject(text)
            | WindowGroupKey::MailAccount(text)
            | WindowGroupKey::Application(text) => text,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowLabel {
    TerminalShell(TerminalShellLabel),
    FirefoxTab {
        page: SplitAppTitle,
        favicon_key: String,
    },
    BrowserTab(SplitAppTitle),
    EditorFile(SplitAppTitle),
    MailFolder(SplitAppTitle),
    Titled(String),
}

impl WindowLabel {
    #[must_use]
    pub fn classify(app_id: &str, title: &str) -> Self {
        let titled = || WindowLabel::Titled(title.to_owned());
        let Ok(app) = RecognisedApp::try_from(app_id) else {
            return titled();
        };
        let Some(split_title) = app.split_title(title) else {
            return titled();
        };
        match app.kind() {
            RecognisedAppKind::Browser => match firefox_favicon_key(title) {
                Some(favicon_key) if app == RecognisedApp::Firefox => WindowLabel::FirefoxTab {
                    page: split_title,
                    favicon_key,
                },
                _ => WindowLabel::BrowserTab(split_title),
            },
            RecognisedAppKind::Editor => WindowLabel::EditorFile(split_title),
            RecognisedAppKind::Mail => WindowLabel::MailFolder(split_title),
        }
    }

    #[must_use]
    pub fn theme_icon(&self) -> Option<String> {
        command_icon(self.program_name()?)
    }

    #[must_use]
    pub fn program_name(&self) -> Option<&str> {
        let WindowLabel::TerminalShell(label) = self else {
            return None;
        };
        match &label.working_location {
            WorkingLocation::RemoteHost(_) => Some(SSH_COMMAND),
            WorkingLocation::LocalDirectory(_) => label.foreground_activity.program_name(),
        }
    }

    #[must_use]
    pub fn favicon_key(&self) -> Option<&str> {
        let WindowLabel::FirefoxTab { favicon_key, .. } = self else {
            return None;
        };
        Some(favicon_key)
    }

    #[must_use]
    pub fn group_key(&self) -> Option<WindowGroupKey<'_>> {
        match self {
            WindowLabel::TerminalShell(label) => Some(match &label.working_location {
                WorkingLocation::LocalDirectory(directory) => {
                    WindowGroupKey::WorkingDirectory(directory)
                }
                WorkingLocation::RemoteHost(host) => WindowGroupKey::RemoteHost(host),
            }),
            WindowLabel::FirefoxTab { page, .. } | WindowLabel::BrowserTab(page) => {
                page.context().map(WindowGroupKey::WebSite)
            }
            WindowLabel::EditorFile(file) => file.context().map(WindowGroupKey::EditorProject),
            WindowLabel::MailFolder(folder) => folder.context().map(WindowGroupKey::MailAccount),
            WindowLabel::Titled(_) => None,
        }
    }

    #[must_use]
    pub fn subject_text(&self) -> &str {
        match self {
            WindowLabel::TerminalShell(label) => label.foreground_activity.display_text(),
            WindowLabel::FirefoxTab { page, .. }
            | WindowLabel::BrowserTab(page)
            | WindowLabel::EditorFile(page)
            | WindowLabel::MailFolder(page) => page.subject(),
            WindowLabel::Titled(title) => title,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_windows_apps::terminal_emulator_windows::terminal_shell_label::ForegroundActivity;

    fn shell_running(command_line: &str) -> TerminalShellLabel {
        TerminalShellLabel {
            working_location: WorkingLocation::LocalDirectory("sottomano".to_owned()),
            foreground_activity: ForegroundActivity::ShellCommand {
                command_line: command_line.to_owned(),
                program_name: None,
            },
        }
    }

    fn shell_theme_icon(label: TerminalShellLabel) -> Option<String> {
        WindowLabel::TerminalShell(label).theme_icon()
    }

    fn shell_idle_in(directory: &str) -> WindowLabel {
        WindowLabel::TerminalShell(TerminalShellLabel {
            working_location: WorkingLocation::LocalDirectory(directory.to_owned()),
            foreground_activity: ForegroundActivity::ShellPrompt {
                shell_name: "nu".to_owned(),
            },
        })
    }

    #[test]
    fn running_and_idle_shells_in_one_directory_share_a_group_key() {
        let running = WindowLabel::TerminalShell(shell_running("cargo"));
        let idle = shell_idle_in("sottomano");
        assert_eq!(running.group_key(), idle.group_key());
        assert_ne!(running.group_key(), shell_idle_in("nix").group_key());
        assert_eq!(idle.subject_text(), "nu");
    }

    #[test]
    fn a_directory_and_a_site_with_one_name_do_not_share_a_group_key() {
        let directory = shell_idle_in("github.com");
        let site = WindowLabel::classify("chromium", "Pull requests - github.com - Chromium");
        assert_eq!(
            site.group_key(),
            Some(WindowGroupKey::WebSite("github.com"))
        );
        assert_ne!(directory.group_key(), site.group_key());
    }

    #[test]
    fn plain_titles_have_no_group_key() {
        let plain = WindowLabel::Titled("Files".to_owned());
        assert_eq!(plain.group_key(), None);
        assert_eq!(plain.subject_text(), "Files");
    }

    #[test]
    fn firefox_windows_carry_their_page_and_favicon_key() {
        assert_eq!(
            WindowLabel::classify("firefox", "Inbox — user@example.com — Mozilla Firefox"),
            WindowLabel::FirefoxTab {
                page: SplitAppTitle::InContext {
                    subject: "Inbox".to_owned(),
                    context: "user@example.com".to_owned(),
                },
                favicon_key: "Inbox — user@example.com".to_owned(),
            }
        );
        assert_eq!(
            WindowLabel::classify("firefox", "about:blank"),
            WindowLabel::Titled("about:blank".to_owned())
        );
    }

    #[test]
    fn branded_browsers_split_and_unknown_app_ids_stay_whole() {
        assert_eq!(
            WindowLabel::classify("org.chromium.Chromium", "Odin-rs · GitHub - Chromium"),
            WindowLabel::BrowserTab(SplitAppTitle::InContext {
                subject: "Odin-rs".to_owned(),
                context: "GitHub".to_owned(),
            })
        );
        assert_eq!(
            WindowLabel::classify("chromium-like", "Odin-rs - Chromium"),
            WindowLabel::Titled("Odin-rs - Chromium".to_owned())
        );
    }

    #[test]
    fn editors_group_by_project_and_mail_by_account() {
        let editor = WindowLabel::classify("code", "README.md - sottomano - Visual Studio Code");
        assert_eq!(
            editor.group_key(),
            Some(WindowGroupKey::EditorProject("sottomano"))
        );
        assert_eq!(editor.subject_text(), "README.md");
        let mail = WindowLabel::classify("thunderbird", "Inbox - Gmail - Mozilla Thunderbird");
        assert_eq!(mail.group_key(), Some(WindowGroupKey::MailAccount("Gmail")));
        assert_eq!(
            WindowLabel::classify("code", "● settings.json - Visual Studio Code").group_key(),
            None
        );
        assert_eq!(
            WindowLabel::classify("thunderbird", "5 Reminders"),
            WindowLabel::Titled("5 Reminders".to_owned())
        );
    }

    #[test]
    fn shell_theme_icon_matches_the_running_command() {
        assert_eq!(
            shell_theme_icon(shell_running("nixos-rebuild")),
            Some("nix-snowflake".to_owned())
        );
        assert_eq!(shell_theme_icon(shell_running("unknown-tool")), None);
        assert_eq!(
            shell_theme_icon(shell_running("./scripts/check.nu")),
            Some("system-run".to_owned())
        );
        assert_eq!(
            shell_theme_icon(TerminalShellLabel {
                working_location: WorkingLocation::RemoteHost("builder".to_owned()),
                foreground_activity: ForegroundActivity::ShellPrompt {
                    shell_name: "nu".to_owned(),
                },
            }),
            Some("gnome-fs-ssh".to_owned())
        );
    }
}
