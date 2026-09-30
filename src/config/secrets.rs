use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct SecretPaths {
    pub openrouter_management: Option<PathBuf>,
    pub claude_code_oauth: Option<PathBuf>,
    pub gh_auth_token: Option<PathBuf>,
    pub unsplash_access_key: Option<PathBuf>,
}

impl SecretPaths {
    pub fn discover(config_home: &Path) -> Self {
        let directory = config_home.join("vetrata");
        let present = |name: &str| Some(directory.join(name)).filter(|path| path.is_file());
        Self {
            openrouter_management: present("openrouter-management-key"),
            claude_code_oauth: present("claude-code-oauth"),
            gh_auth_token: present("gh-auth-token"),
            unsplash_access_key: present("unsplash-access-key"),
        }
    }
}
