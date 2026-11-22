use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecretsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openrouter_management_key: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code_oauth: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gh_auth_token: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsplash_access_key: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caldav_password: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tailscale_api_token: Option<PathBuf>,
}

impl SecretsConfig {
    #[must_use]
    pub fn configured_path(&self, secret: SecretFile) -> Option<&Path> {
        match secret {
            SecretFile::OpenrouterManagement => self.openrouter_management_key.as_deref(),
            SecretFile::ClaudeCodeOauth => self.claude_code_oauth.as_deref(),
            SecretFile::GhAuthToken => self.gh_auth_token.as_deref(),
            SecretFile::UnsplashAccessKey => self.unsplash_access_key.as_deref(),
            SecretFile::CaldavPassword => self.caldav_password.as_deref(),
            SecretFile::TailscaleApiToken => self.tailscale_api_token.as_deref(),
        }
    }
}

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
    pub fn config_key(self) -> &'static str {
        match self {
            SecretFile::OpenrouterManagement => "openrouter_management_key",
            SecretFile::ClaudeCodeOauth => "claude_code_oauth",
            SecretFile::GhAuthToken => "gh_auth_token",
            SecretFile::UnsplashAccessKey => "unsplash_access_key",
            SecretFile::CaldavPassword => "caldav_password",
            SecretFile::TailscaleApiToken => "tailscale_api_token",
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
    pub fn stored_in(self, secrets_config: &SecretsConfig, config_home: &Path) -> StoredSecret {
        let Some(configured_path) = secrets_config.configured_path(self) else {
            return StoredSecret::Absent;
        };
        let resolved_path = config_home.join(configured_path);
        if resolved_path.is_file() {
            StoredSecret::Present(resolved_path)
        } else {
            tracing::warn!(
                "secrets.{} points at {}, which is not a readable file",
                self.config_key(),
                resolved_path.display()
            );
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
    pub fn discover(secrets_config: &SecretsConfig, config_home: &Path) -> Self {
        Self {
            openrouter_management: SecretFile::OpenrouterManagement
                .stored_in(secrets_config, config_home),
            claude_code_oauth: SecretFile::ClaudeCodeOauth.stored_in(secrets_config, config_home),
            gh_auth_token: SecretFile::GhAuthToken.stored_in(secrets_config, config_home),
            unsplash_access_key: SecretFile::UnsplashAccessKey
                .stored_in(secrets_config, config_home),
            caldav_password: SecretFile::CaldavPassword.stored_in(secrets_config, config_home),
            tailscale_api_token: SecretFile::TailscaleApiToken
                .stored_in(secrets_config, config_home),
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

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf, process};

    use super::{SecretFile, SecretPaths, SecretsConfig, StoredSecret};

    fn scratch_config_home(test_name: &str) -> PathBuf {
        let config_home =
            env::temp_dir().join(format!("sottomano-secrets-{test_name}-{}", process::id()));
        fs::create_dir_all(&config_home).unwrap();
        config_home
    }

    #[test]
    fn a_secrets_table_parses_into_paths() {
        let parsed: SecretsConfig =
            toml::from_str("caldav_password = \"/run/agenix/caldav-password\"\n").unwrap();
        assert_eq!(
            parsed.configured_path(SecretFile::CaldavPassword),
            Some(PathBuf::from("/run/agenix/caldav-password").as_path())
        );
        assert_eq!(parsed.configured_path(SecretFile::GhAuthToken), None);
    }

    #[test]
    fn an_unknown_secret_key_is_rejected() {
        let parsed = toml::from_str::<SecretsConfig>("calendar_password = \"/tmp/x\"\n");
        assert!(parsed.is_err());
    }

    #[test]
    fn a_relative_path_resolves_against_the_config_home() {
        let config_home = scratch_config_home("relative");
        fs::write(config_home.join("tailnet-token"), "token").unwrap();
        let secrets_config = SecretsConfig {
            tailscale_api_token: Some(PathBuf::from("tailnet-token")),
            ..SecretsConfig::default()
        };
        let discovered_paths = SecretPaths::discover(&secrets_config, &config_home);
        fs::remove_dir_all(&config_home).unwrap();
        assert_eq!(
            discovered_paths.tailscale_api_token,
            StoredSecret::Present(config_home.join("tailnet-token"))
        );
    }

    #[test]
    fn a_configured_but_missing_file_is_absent() {
        let config_home = scratch_config_home("missing");
        let secrets_config = SecretsConfig {
            gh_auth_token: Some(config_home.join("does-not-exist")),
            ..SecretsConfig::default()
        };
        let discovered_paths = SecretPaths::discover(&secrets_config, &config_home);
        fs::remove_dir_all(&config_home).unwrap();
        assert_eq!(discovered_paths.gh_auth_token, StoredSecret::Absent);
    }
}
