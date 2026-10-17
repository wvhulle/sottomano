use crate::data::source::window_agents::{
    KITTY_APP_ID,
    kitty_window_label::{ForegroundActivity, KittyWindowLabel, WorkingLocation},
};

const SEPARATORS: [&str; 3] = [" — ", " · ", " - "];

const FIREFOX_APP_IDS: [&str; 2] = ["firefox", "org.mozilla.firefox"];

const FIREFOX_BRANDS: [&str; 2] = ["Mozilla Firefox", "Firefox"];

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPage {
    pub page: String,
    pub site: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBrand {
    LibreWolf,
    Chromium,
    GoogleChrome,
    Brave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnbrandedApp;

impl TryFrom<&str> for BrowserBrand {
    type Error = UnbrandedApp;

    fn try_from(app_id: &str) -> Result<Self, Self::Error> {
        match app_id {
            "librewolf" | "io.gitlab.librewolf-community" => Ok(BrowserBrand::LibreWolf),
            "chromium" | "chromium-browser" | "org.chromium.Chromium" => Ok(BrowserBrand::Chromium),
            "google-chrome" | "com.google.Chrome" => Ok(BrowserBrand::GoogleChrome),
            "brave-browser" | "com.brave.Browser" => Ok(BrowserBrand::Brave),
            _ => Err(UnbrandedApp),
        }
    }
}

impl BrowserBrand {
    fn title_suffix(self) -> &'static str {
        match self {
            BrowserBrand::LibreWolf => "LibreWolf",
            BrowserBrand::Chromium => "Chromium",
            BrowserBrand::GoogleChrome => "Google Chrome",
            BrowserBrand::Brave => "Brave",
        }
    }
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
            let Some(page) = BrowserPage::branded(title, &FIREFOX_BRANDS) else {
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
        BrowserPage::branded(title, &[brand.title_suffix()])
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

impl BrowserPage {
    fn branded(title: &str, brands: &[&str]) -> Option<Self> {
        let page = page_title(title, brands).filter(|page| !page.is_empty())?;
        let Some(site_separator) = last_separator(page) else {
            return Some(BrowserPage {
                page: page.to_owned(),
                site: None,
            });
        };
        Some(BrowserPage {
            page: page[..site_separator.at].to_owned(),
            site: Some(page[site_separator.at + site_separator.length..].to_owned()),
        })
    }
}

pub fn firefox_favicon_key(title: &str) -> Option<String> {
    let without_badge = strip_unread_badge(page_title(title, &FIREFOX_BRANDS)?);
    (!without_badge.is_empty()).then(|| without_badge.to_owned())
}

fn page_title<'a>(title: &'a str, brands: &[&str]) -> Option<&'a str> {
    brands.iter().find_map(|brand| {
        SEPARATORS
            .iter()
            .find_map(|separator| title.strip_suffix(&format!("{separator}{brand}")))
    })
}

pub fn strip_unread_badge(title: &str) -> &str {
    let Some(rest) = title.strip_prefix('(') else {
        return title;
    };
    let Some((count, tail)) = rest.split_once(") ") else {
        return title;
    };
    let digits = count.strip_suffix('+').unwrap_or(count);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return title;
    }
    tail
}

struct SeparatorMatch {
    at: usize,
    length: usize,
}

fn last_separator(text: &str) -> Option<SeparatorMatch> {
    SEPARATORS
        .iter()
        .filter_map(|separator| {
            Some(SeparatorMatch {
                at: text.rfind(separator)?,
                length: separator.len(),
            })
        })
        .max_by_key(|found| found.at)
}

const CMD_ICONS: &[(&str, &str)] = &[
    ("nvim", "nvim"),
    ("neovim", "nvim"),
    ("vim", "vim"),
    ("hx", "helix"),
    ("helix", "helix"),
    ("emacs", "emacs"),
    ("nano", "text-editor"),
    ("micro", "text-editor"),
    ("code", "visual-studio-code"),
    ("codium", "visual-studio-code"),
    ("zed", "text-editor"),
    ("git", "git"),
    ("jj", "gitg"),
    ("jjj", "gitg"),
    ("jjui", "gitg"),
    ("lazygit", "git-cola"),
    ("lg", "git-cola"),
    ("gitui", "git-cola"),
    ("tig", "git-cola"),
    ("convco", "git"),
    ("onefetch", "git"),
    ("rad", "git"),
    ("pre-commit", "git"),
    ("gh", "github-desktop"),
    ("glab", "github-desktop"),
    ("fj", "github-desktop"),
    ("claude", "claude"),
    ("opencode", "copilot"),
    ("aider", "copilot"),
    ("ollama", "ollama"),
    ("cargo", "applications-development"),
    ("rustc", "applications-development"),
    ("rustup", "applications-development"),
    ("rust-analyzer", "applications-development"),
    ("bacon", "applications-development"),
    ("clippy", "applications-development"),
    ("nix", "nix-snowflake"),
    ("nh", "nix-snowflake"),
    ("nom", "nix-snowflake"),
    ("nix-shell", "nix-snowflake"),
    ("nix-build", "nix-snowflake"),
    ("nix-store", "nix-snowflake"),
    ("nix-tree", "nix-snowflake"),
    ("nix-locate", "nix-snowflake"),
    ("nix-prefetch-github", "nix-snowflake"),
    ("nixos-rebuild", "nix-snowflake"),
    ("home-manager", "nix-snowflake"),
    ("home-switch", "nix-snowflake"),
    ("system-switch", "nix-snowflake"),
    ("statix", "nix-snowflake"),
    ("deadnix", "nix-snowflake"),
    ("nixfmt", "nix-snowflake"),
    ("agenix", "nix-snowflake"),
    ("python", "python"),
    ("python3", "python"),
    ("ipython", "python"),
    ("uv", "python"),
    ("pip", "python"),
    ("pipx", "python"),
    ("ruff", "python"),
    ("node", "applications-development"),
    ("npm", "applications-development"),
    ("npx", "applications-development"),
    ("pnpm", "applications-development"),
    ("yarn", "applications-development"),
    ("bun", "applications-development"),
    ("deno", "applications-development"),
    ("go", "applications-development"),
    ("make", "applications-development"),
    ("cmake", "applications-development"),
    ("ninja", "applications-development"),
    ("conan", "package-x-generic"),
    ("typst", "latex"),
    ("tectonic", "latex"),
    ("latexmk", "latex"),
    ("pdflatex", "latex"),
    ("tree-sitter", "applications-development"),
    ("treefmt", "applications-development"),
    ("tokei", "applications-development"),
    ("docker", "docker"),
    ("docker-compose", "docker"),
    ("lazydocker", "docker"),
    ("podman", "ui-container-podman-symbolic"),
    ("k9s", "container-terminal-symbolic"),
    ("kubectl", "container-terminal-symbolic"),
    ("helm", "container-terminal-symbolic"),
    ("sudo", "security-high"),
    ("doas", "security-high"),
    ("systemctl", "applications-system"),
    ("systemd-analyze", "applications-system"),
    ("loginctl", "applications-system"),
    ("journalctl", "logviewer"),
    ("dmesg", "logviewer"),
    ("reboot", "system-reboot"),
    ("poweroff", "system-shutdown"),
    ("shutdown", "system-shutdown"),
    ("nmcli", "network-manager"),
    ("nmtui", "network-manager"),
    ("mmcli", "modem"),
    ("bluetoothctl", "preferences-system-bluetooth"),
    ("powerprofilesctl", "preferences-system-power"),
    ("brightnessctl", "display-brightness"),
    ("solar-brightness", "display-brightness"),
    ("darkman", "display-brightness"),
    ("niri", "preferences-desktop-display"),
    ("playerctl", "multimedia-player"),
    ("spotify_player", "multimedia-player"),
    ("ncmpcpp", "multimedia-player"),
    ("mpv", "multimedia-player"),
    ("pactl", "audio-volume-high"),
    ("wpctl", "audio-volume-high"),
    ("alsamixer", "audio-volume-high"),
    ("pavucontrol", "audio-volume-high"),
    ("tailscale", "network-vpn"),
    ("wg", "network-vpn"),
    ("wg-quick", "network-vpn"),
    ("ssh", "gnome-fs-ssh"),
    ("scp", "gnome-fs-ssh"),
    ("sftp", "gnome-fs-ssh"),
    ("rsync", "gnome-fs-ssh"),
    ("ssh-add", "gnome-fs-ssh"),
    ("ssh-keygen", "gnome-fs-ssh"),
    ("curl", "applications-internet"),
    ("wget", "applications-internet"),
    ("xh", "applications-internet"),
    ("xdg-open", "web-browser"),
    ("ping", "network-wired"),
    ("traceroute", "network-wired"),
    ("dig", "network-wired"),
    ("gpg", "gnupg"),
    ("pass", "password-manager"),
    ("notify-send", "dialog-information"),
    ("watch", "clock"),
    ("man", "system-help"),
    ("tldr", "system-help"),
    ("jira", "applications-office"),
    ("jiratui", "applications-office"),
    ("htop", "htop"),
    ("top", "htop"),
    ("btop", "btop"),
    ("btm", "btop"),
    ("bpytop", "btop"),
    ("ps", "utilities-system-monitor"),
    ("dust", "disk-usage-analyzer"),
    ("ncdu", "disk-usage-analyzer"),
    ("du", "disk-usage-analyzer"),
    ("df", "disk-usage-analyzer"),
    ("duf", "disk-usage-analyzer"),
    ("lsblk", "drive-harddisk"),
    ("fdisk", "drive-harddisk"),
    ("gparted", "drive-harddisk"),
    ("ranger", "system-file-manager"),
    ("yazi", "system-file-manager"),
    ("lf", "system-file-manager"),
    ("nnn", "system-file-manager"),
    ("broot", "system-file-manager"),
    ("tree", "system-file-manager"),
    ("eza", "system-file-manager"),
    ("lsd", "system-file-manager"),
    ("tar", "utilities-file-archiver"),
    ("zip", "utilities-file-archiver"),
    ("unzip", "utilities-file-archiver"),
    ("7z", "utilities-file-archiver"),
    ("rg", "edit-find"),
    ("grep", "edit-find"),
    ("fd", "edit-find"),
    ("find", "edit-find"),
    ("ast-grep", "edit-find"),
    ("sg", "edit-find"),
    ("scooter", "edit-find"),
    ("fzf", "edit-find"),
    ("zellij", "utilities-terminal"),
    ("tmux", "utilities-terminal"),
    ("screen", "utilities-terminal"),
    ("steam", "steam"),
    ("spotify", "spotify"),
];

const SCRIPT_ICONS: &[(&str, &str)] = &[
    (".nu", "system-run"),
    (".sh", "system-run"),
    (".py", "python"),
];

fn command_icon(command: &str) -> Option<String> {
    CMD_ICONS
        .iter()
        .find(|(cmd, _)| *cmd == command)
        .or_else(|| {
            SCRIPT_ICONS
                .iter()
                .find(|(extension, _)| command.ends_with(*extension))
        })
        .map(|(_, icon)| (*icon).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(page: &str, site: Option<&str>) -> BrowserPage {
        BrowserPage {
            page: page.to_owned(),
            site: site.map(str::to_owned),
        }
    }

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
    fn firefox_titles_split_into_page_and_site() {
        assert_eq!(
            WindowLabel::classify(
                Some("firefox"),
                "Inbox — user@example.com — Mozilla Firefox",
                None
            ),
            WindowLabel::Firefox {
                page: page("Inbox", Some("user@example.com")),
                favicon_key: Some("Inbox — user@example.com".to_owned()),
            }
        );
    }

    #[test]
    fn firefox_titles_without_the_brand_stay_whole() {
        assert_eq!(
            WindowLabel::classify(Some("firefox"), "about:blank", None),
            WindowLabel::Titled("about:blank".to_owned())
        );
    }

    #[test]
    fn firefox_favicon_key_ignores_unread_badges() {
        assert_eq!(
            firefox_favicon_key("(2) WhatsApp — Mozilla Firefox"),
            Some("WhatsApp".to_owned())
        );
        assert_eq!(
            firefox_favicon_key("(20+) Facebook — Firefox"),
            Some("Facebook".to_owned())
        );
        assert_eq!(
            firefox_favicon_key("(draft) Notes — Firefox"),
            Some("(draft) Notes".to_owned())
        );
        assert_eq!(firefox_favicon_key("some plain title"), None);
    }

    #[test]
    fn branded_browsers_are_classified_by_exact_app_id() {
        assert_eq!(
            WindowLabel::classify(
                Some("org.chromium.Chromium"),
                "Odin-rs · GitHub - Chromium",
                None
            ),
            WindowLabel::BrandedBrowser(page("Odin-rs", Some("GitHub")))
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
