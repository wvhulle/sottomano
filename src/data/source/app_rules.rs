use super::niri::WinEntry;

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

#[derive(Debug, Clone, Default)]
pub struct TitleParts {
    pub lead: String,
    pub main: String,
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

    pub fn favicon_key(self, title: &str) -> Option<String> {
        match self {
            AppRule::Firefox => core(title, &FIREFOX_BRANDS).filter(|c| !c.is_empty()),
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
    ("jj", "gitg"),
    ("lazygit", "git-cola"),
    ("tig", "git-cola"),
    ("gh", "github-desktop"),
    ("claude", "claude"),
    ("ollama", "ollama"),
    ("htop", "htop"),
    ("btop", "btop"),
    ("btm", "btop"),
    ("k9s", "kubernetes"),
    ("docker", "docker"),
    ("podman", "podman"),
    ("cargo", "rust"),
    ("nix", "nix-snowflake"),
    ("nh", "nix-snowflake"),
    ("ranger", "system-file-manager"),
    ("yazi", "system-file-manager"),
    ("lf", "system-file-manager"),
];

fn terminal_icon(win: &WinEntry) -> Option<String> {
    if is_claude_session(&win.title) {
        return Some("claude".to_string());
    }
    let main = AppRule::Terminal.rewrite(&win.title).main;
    let word = main.split_whitespace().next()?;
    CMD_ICONS
        .iter()
        .find(|(cmd, _)| *cmd == word)
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
    use super::*;

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
