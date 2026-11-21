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
                 indicator_groups.openrouter_credit.max_blended_usd_per_million."
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
                 → Security). Shows tasks and upcoming events of indicator_groups.caldav."
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
    pub fn stored_in(self, config_home: &Path) -> StoredSecret {
        let path = Self::directory(config_home).join(self.file_name());
        if path.is_file() {
            StoredSecret::Present(path)
        } else {
            StoredSecret::Absent
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum StoredSecret {
    #[default]
    Absent,
    Present(PathBuf),
}

#[derive(Debug, Clone, Default)]
pub struct SecretPaths {
    pub openrouter_management: StoredSecret,
    pub claude_code_oauth: StoredSecret,
    pub gh_auth_token: StoredSecret,
    pub unsplash_access_key: StoredSecret,
    pub caldav_password: StoredSecret,
    pub tailscale_api_token: StoredSecret,
}

impl SecretPaths {
    #[must_use]
    pub fn discover(config_home: &Path) -> Self {
        Self {
            openrouter_management: SecretFile::OpenrouterManagement.stored_in(config_home),
            claude_code_oauth: SecretFile::ClaudeCodeOauth.stored_in(config_home),
            gh_auth_token: SecretFile::GhAuthToken.stored_in(config_home),
            unsplash_access_key: SecretFile::UnsplashAccessKey.stored_in(config_home),
            caldav_password: SecretFile::CaldavPassword.stored_in(config_home),
            tailscale_api_token: SecretFile::TailscaleApiToken.stored_in(config_home),
        }
    }

    #[must_use]
    pub fn stored(&self, secret: SecretFile) -> &StoredSecret {
        match secret {
            SecretFile::OpenrouterManagement => &self.openrouter_management,
            SecretFile::ClaudeCodeOauth => &self.claude_code_oauth,
            SecretFile::GhAuthToken => &self.gh_auth_token,
            SecretFile::UnsplashAccessKey => &self.unsplash_access_key,
            SecretFile::CaldavPassword => &self.caldav_password,
            SecretFile::TailscaleApiToken => &self.tailscale_api_token,
        }
    }
}
