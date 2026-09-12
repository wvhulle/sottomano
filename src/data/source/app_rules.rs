use crate::data::source::niri::WinEntry;

const SEPARATORS: [&str; 3] = [" — ", " · ", " - "];

const TERMINAL_KEYWORDS: [&str; 5] = ["kitty", "foot", "alacritty", "wezterm", "ghostty"];

const FIREFOX_BRANDS: [&str; 2] = ["Mozilla Firefox", "Firefox"];

#[derive(Debug, Clone, Copy)]
pub enum AppRule {
    Terminal,
    Firefox,
    Browser { brand: &'static str },
    Fallback,
}

pub fn rule_for(app_id: &str) -> AppRule {
    let id = app_id.to_lowercase();
    if TERMINAL_KEYWORDS.iter().any(|k| id.contains(k)) {
        AppRule::Terminal
    } else if id.contains("firefox") {
        AppRule::Firefox
    } else if id.contains("librewolf") {
        AppRule::Browser { brand: "LibreWolf" }
    } else if id.contains("chromium") {
        AppRule::Browser { brand: "Chromium" }
    } else if id.contains("chrome") {
        AppRule::Browser {
            brand: "Google Chrome",
        }
    } else if id.contains("brave") {
        AppRule::Browser { brand: "Brave" }
    } else {
        AppRule::Fallback
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleParts {
    pub lead: String,
    pub main: String,
}

pub fn label_for(win: &WinEntry) -> &str {
    if win.title.is_empty() {
        &win.app_id
    } else {
        &win.title
    }
}

#[derive(Debug, Clone)]
pub enum IconCandidate {
    Theme(String),
    AppId(String),
}

impl AppRule {
    pub fn rewrite(self, title: &str) -> TitleParts {
        match self {
            AppRule::Terminal => split(title, false).map_or_else(
                || TitleParts {
                    lead: String::new(),
                    main: title.to_string(),
                },
                |(head, tail)| TitleParts {
                    lead: head.to_string(),
                    main: tail.to_string(),
                },
            ),
            AppRule::Firefox => browser_rewrite(title, &FIREFOX_BRANDS),
            AppRule::Browser { brand } => browser_rewrite(title, &[brand]),
            AppRule::Fallback => TitleParts {
                lead: String::new(),
                main: title.to_string(),
            },
        }
    }

    pub fn lead_first(self) -> bool {
        !matches!(self, AppRule::Terminal)
    }

    pub fn favicon_key(self, title: &str) -> Option<String> {
        match self {
            AppRule::Firefox => core(title, &FIREFOX_BRANDS)
                .map(|c| strip_unread_badge(&c).to_string())
                .filter(|c| !c.is_empty()),
            _ => None,
        }
    }

    pub fn icon_candidates(self, win: &WinEntry) -> Vec<IconCandidate> {
        match self {
            AppRule::Firefox => vec![IconCandidate::AppId(win.app_id.clone())],
            AppRule::Terminal => {
                let mut candidates = Vec::with_capacity(2);
                if let Some(theme) = terminal_icon(win) {
                    candidates.push(IconCandidate::Theme(theme));
                }
                candidates.push(IconCandidate::AppId(win.app_id.clone()));
                candidates
            }
            AppRule::Browser { .. } | AppRule::Fallback => {
                vec![IconCandidate::AppId(win.app_id.clone())]
            }
        }
    }
}

fn browser_rewrite(title: &str, brands: &[&str]) -> TitleParts {
    match core(title, brands).filter(|c| !c.is_empty()) {
        Some(c) => split(&c, true).map_or_else(
            || TitleParts {
                lead: String::new(),
                main: c.clone(),
            },
            |(head, tail)| TitleParts {
                lead: tail.to_string(),
                main: head.to_string(),
            },
        ),
        None => TitleParts {
            lead: String::new(),
            main: title.to_string(),
        },
    }
}

fn core(title: &str, brands: &[&str]) -> Option<String> {
    brands.iter().find_map(|brand| {
        SEPARATORS
            .iter()
            .find_map(|sep| title.strip_suffix(&format!("{sep}{brand}")))
            .map(str::to_string)
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

fn split(s: &str, from_end: bool) -> Option<(&str, &str)> {
    let mut best: Option<(usize, usize)> = None;
    for sep in SEPARATORS {
        let Some(at) = (if from_end { s.rfind(sep) } else { s.find(sep) }) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((b, _)) => {
                if from_end {
                    at > b
                } else {
                    at < b
                }
            }
        };
        if better {
            best = Some((at, sep.len()));
        }
    }
    best.map(|(at, len)| (&s[..at], &s[at + len..]))
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

fn terminal_icon(win: &WinEntry) -> Option<String> {
    if is_claude_session(&win.title) {
        return Some("claude".to_string());
    }
    command_icon(&AppRule::Terminal.rewrite(&win.title).main)
}

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

fn is_claude_session(title: &str) -> bool {
    title
        .trim()
        .chars()
        .next()
        .is_some_and(|c| c == '\u{2733}' || ('\u{2800}'..='\u{28FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::{super::niri::WinLayout, *};

    #[test]
    fn firefox_title_rewrite() {
        let parts = AppRule::Firefox.rewrite("Inbox — user@example.com — Mozilla Firefox");
        assert_eq!(parts.lead, "user@example.com");
        assert_eq!(parts.main, "Inbox");
    }

    #[test]
    fn firefox_no_brand_suffix_falls_back() {
        let parts = AppRule::Firefox.rewrite("about:blank");
        assert_eq!(parts.lead, "");
        assert_eq!(parts.main, "about:blank");
    }

    #[test]
    fn terminal_title_rewrite() {
        let parts = AppRule::Terminal.rewrite("~/Code/odin-rs — nvim src/main.rs");
        assert_eq!(parts.lead, "~/Code/odin-rs");
        assert_eq!(parts.main, "nvim src/main.rs");
    }

    #[test]
    fn firefox_favicon_key_matches_core() {
        assert_eq!(
            AppRule::Firefox.favicon_key("Odin-rs · GitHub — Firefox"),
            Some("Odin-rs · GitHub".to_string())
        );
        assert_eq!(AppRule::Firefox.favicon_key("some plain title"), None);
    }

    #[test]
    fn firefox_favicon_key_ignores_unread_badges() {
        assert_eq!(
            AppRule::Firefox.favicon_key("(2) WhatsApp — Mozilla Firefox"),
            Some("WhatsApp".to_string())
        );
        assert_eq!(
            AppRule::Firefox.favicon_key("(20+) Facebook — Firefox"),
            Some("Facebook".to_string())
        );
    }

    #[test]
    fn firefox_favicon_key_keeps_non_numeric_parentheses() {
        assert_eq!(
            AppRule::Firefox.favicon_key("(draft) Notes — Firefox"),
            Some("(draft) Notes".to_string())
        );
        assert_eq!(
            AppRule::Firefox.favicon_key("(1)WhatsApp — Firefox"),
            Some("(1)WhatsApp".to_string())
        );
    }

    fn kitty_window(title: &str) -> WinEntry {
        WinEntry {
            id: 1,
            title: title.to_string(),
            app_id: "kitty".to_string(),
            pid: None,
            is_focused: false,
            is_urgent: false,
            layout: WinLayout::default(),
            title_parts: TitleParts::default(),
            lead_first: false,
        }
    }

    #[test]
    fn terminals_put_the_command_before_the_directory() {
        let rule = rule_for("kitty");
        let parts = rule.rewrite("vetrata — cargo");
        assert_eq!(parts.lead, "vetrata");
        assert_eq!(parts.main, "cargo");
        assert!(!rule.lead_first());
    }

    #[test]
    fn browsers_strip_the_brand_and_lead_with_context() {
        let rule = rule_for("firefox");
        let parts = rule.rewrite("Odin-rs · GitHub — Mozilla Firefox");
        assert_eq!(parts.main, "Odin-rs");
        assert_eq!(parts.lead, "GitHub");
        assert!(rule.lead_first());
    }

    #[test]
    fn terminal_icon_matches_the_command_word() {
        assert_eq!(
            terminal_icon(&kitty_window("nixos-config — nixos-rebuild")),
            Some("nix-snowflake".to_string())
        );
        assert_eq!(
            terminal_icon(&kitty_window("vetrata — cargo")),
            Some("applications-development".to_string())
        );
        assert_eq!(
            terminal_icon(&kitty_window("nixos-config — sudo")),
            Some("security-high".to_string())
        );
        assert_eq!(terminal_icon(&kitty_window("vetrata — unknown-tool")), None);
    }

    #[test]
    fn terminal_icon_matches_script_extensions() {
        assert_eq!(
            terminal_icon(&kitty_window("nixos-config — ./scripts/check.nu")),
            Some("system-run".to_string())
        );
    }

    #[test]
    fn rule_for_prefers_more_specific_keyword() {
        assert!(matches!(
            rule_for("org.chromium.Chromium"),
            AppRule::Browser { brand: "Chromium" }
        ));
        assert!(matches!(
            rule_for("google-chrome"),
            AppRule::Browser {
                brand: "Google Chrome"
            }
        ));
    }
}
