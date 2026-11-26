use std::{
    env,
    path::{Path, PathBuf},
};

use crate::system_protocol_clients::sysfs_attribute_reads;

const STEAM_ROOT_CANDIDATES: [&str; 2] = [".local/share/Steam", ".steam/steam"];
const STEAM_PID_FILE: &str = ".steam/steam.pid";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SteamRoot {
    pub root_directory: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamClientPresence {
    Running,
    Stopped,
}

fn home_directory() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

impl SteamRoot {
    #[must_use]
    pub fn located() -> Option<Self> {
        let home = home_directory()?;
        STEAM_ROOT_CANDIDATES
            .iter()
            .map(|candidate| home.join(candidate))
            .find(|root_directory| root_directory.join("config").is_dir())
            .map(|root_directory| SteamRoot { root_directory })
    }

    #[must_use]
    pub fn log_file(&self, log_name: &str) -> PathBuf {
        self.root_directory.join("logs").join(log_name)
    }

    #[must_use]
    pub fn config_file(&self, file_name: &str) -> PathBuf {
        self.root_directory.join("config").join(file_name)
    }

    #[must_use]
    pub fn app_info_file(&self) -> PathBuf {
        self.root_directory.join("appcache").join("appinfo.vdf")
    }

    #[must_use]
    pub fn library_folders_file(&self) -> PathBuf {
        self.root_directory
            .join("steamapps")
            .join("libraryfolders.vdf")
    }
}

impl SteamClientPresence {
    #[must_use]
    pub fn current() -> Self {
        let Some(home) = home_directory() else {
            return SteamClientPresence::Stopped;
        };
        let Some(process_id) = sysfs_attribute_reads::read_i64(home.join(STEAM_PID_FILE)) else {
            return SteamClientPresence::Stopped;
        };
        if Path::new(&format!("/proc/{process_id}")).exists() {
            SteamClientPresence::Running
        } else {
            SteamClientPresence::Stopped
        }
    }
}
