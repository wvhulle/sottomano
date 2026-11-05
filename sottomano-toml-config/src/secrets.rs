use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretFile {
    OpenrouterManagement,
    ClaudeCodeOauth,
    GhAuthToken,
    UnsplashAccessKey,
    CaldavPassword,
    TailscaleApiToken,
}

impl SecretFile {
    pub const ALL: [SecretFile; 6] = [
        SecretFile::OpenrouterManagement,
        SecretFile::ClaudeCodeOauth,
        SecretFile::GhAuthToken,
        SecretFile::UnsplashAccessKey,
        SecretFile::CaldavPassword,
        SecretFile::TailscaleApiToken,
    ];

    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            SecretFile::OpenrouterManagement => "openrouter-management-key",
            SecretFile::ClaudeCodeOauth => "claude-code-oauth",
            SecretFile::GhAuthToken => "gh-auth-token",
            SecretFile::UnsplashAccessKey => "unsplash-access-key",
            SecretFile::CaldavPassword => "caldav-password",
            SecretFile::TailscaleApiToken => "tailscale-api-token",
        }
    }

    #[must_use]
    pub fn purpose(self) -> &'static str {
        match self {
            SecretFile::OpenrouterManagement => {
                "OpenRouter management key, plain text; a regular API key cannot call \
                 /analytics/query. Shows OpenRouter spend and ranks tool-capable OpenRouter models \
                 by Intelligence Index, within \
                 widget_groups.openrouter_credit.max_blended_usd_per_million."
            }
            SecretFile::ClaudeCodeOauth => {
                "Claude Code credentials JSON (~/.claude/.credentials.json). Pro or Max shows the \
                 usage windows; Enterprise shows spend from ccusage."
            }
            SecretFile::GhAuthToken => {
                "Bare gho_ GitHub OAuth token for /copilot_internal/user. Shows the Copilot quota."
            }
            SecretFile::UnsplashAccessKey => {
                "Unsplash access key (the app's Client-ID, never the secret key). Fetches \
                 wallpapers."
            }
            SecretFile::CaldavPassword => {
                "CalDAV password, plain text; prefer an app password (Nextcloud: Personal settings \
                 → Security). Shows tasks and upcoming events of widget_groups.caldav."
            }
            SecretFile::TailscaleApiToken => {
                "Tailscale API access token, plain text. Shows the tailnet policy file in the \
                 Tailscale access hover card."
            }
        }
    }

    #[must_use]
    pub fn directory(config_home: &Path) -> PathBuf {
        config_home.join("sottomano")
    }

    #[must_use]
    pub fn present_in(self, config_home: &Path) -> Option<PathBuf> {
        Some(Self::directory(config_home).join(self.file_name())).filter(|path| path.is_file())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SecretPaths {
    pub openrouter_management: Option<PathBuf>,
    pub claude_code_oauth: Option<PathBuf>,
    pub gh_auth_token: Option<PathBuf>,
    pub unsplash_access_key: Option<PathBuf>,
    pub caldav_password: Option<PathBuf>,
    pub tailscale_api_token: Option<PathBuf>,
}

impl SecretPaths {
    #[must_use]
    pub fn discover(config_home: &Path) -> Self {
        Self {
            openrouter_management: SecretFile::OpenrouterManagement.present_in(config_home),
            claude_code_oauth: SecretFile::ClaudeCodeOauth.present_in(config_home),
            gh_auth_token: SecretFile::GhAuthToken.present_in(config_home),
            unsplash_access_key: SecretFile::UnsplashAccessKey.present_in(config_home),
            caldav_password: SecretFile::CaldavPassword.present_in(config_home),
            tailscale_api_token: SecretFile::TailscaleApiToken.present_in(config_home),
        }
    }

    #[must_use]
    pub fn contains(&self, secret: SecretFile) -> bool {
        match secret {
            SecretFile::OpenrouterManagement => self.openrouter_management.is_some(),
            SecretFile::ClaudeCodeOauth => self.claude_code_oauth.is_some(),
            SecretFile::GhAuthToken => self.gh_auth_token.is_some(),
            SecretFile::UnsplashAccessKey => self.unsplash_access_key.is_some(),
            SecretFile::CaldavPassword => self.caldav_password.is_some(),
            SecretFile::TailscaleApiToken => self.tailscale_api_token.is_some(),
        }
    }
}
