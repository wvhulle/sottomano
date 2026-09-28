use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unreadable(String),
    Unparsable(String),
    Panicked,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Unreadable(path) => write!(f, "cannot read {path}"),
            Error::Unparsable(path) => write!(f, "unexpected contents in {path}"),
            Error::Panicked => write!(f, "sensor read panicked"),
        }
    }
}

pub(super) fn read_field(path: &Path) -> Result<String, Error> {
    read_trimmed(path).ok_or_else(|| Error::Unreadable(path.display().to_string()))
}

pub(super) fn parse_field<T: FromStr>(path: &Path) -> Result<T, Error> {
    read_field(path)?
        .parse()
        .map_err(|_| Error::Unparsable(path.display().to_string()))
}

pub mod battery;
#[allow(dead_code)]
pub mod cpu_clock;
pub mod cpu_usage;
pub mod memory;
pub mod temperature;

pub(super) const CPU_FREQ: &str = "/sys/devices/system/cpu";

pub(super) fn core_freqs() -> Vec<cpu_clock::CoreFreq> {
    let mut cores: Vec<cpu_clock::CoreFreq> = list_dir(CPU_FREQ)
        .iter()
        .filter_map(|dir| {
            let name = dir.file_name()?.to_str()?;
            let core: u32 = cpu_core_index(name)?;
            let khz = read_i64(dir.join("cpufreq/scaling_cur_freq"))?;
            Some(cpu_clock::CoreFreq {
                core,
                ghz: khz as f64 / 1_000_000.0,
            })
        })
        .collect();
    cores.sort_by_key(|c| c.core);
    cores
}

pub(super) fn core_ghz() -> Vec<f64> {
    core_freqs().iter().map(|c| c.ghz).collect()
}

fn cpu_core_index(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("cpu")?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

pub(super) const POWER_SUPPLY: &str = "/sys/class/power_supply";

pub(super) fn first_power_supply(prefix: &str) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = list_dir(POWER_SUPPLY)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        })
        .collect();
    entries.sort();
    entries.into_iter().next()
}

pub fn internal_brightness_frac() -> Option<f64> {
    let panel = first_backlight()?;
    let current = read_i64(panel.join("brightness"))?;
    let max = read_i64(panel.join("max_brightness")).filter(|m| *m > 0)?;
    Some((current as f64 / max as f64).clamp(0.0, 1.0))
}

fn first_backlight() -> Option<PathBuf> {
    let mut panels = list_dir("/sys/class/backlight");
    panels.sort();
    panels.into_iter().next()
}

pub(super) fn list_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect()
}

pub(super) fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    Some(fs::read_to_string(path).ok()?.trim().to_string())
}

pub(super) fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    read_trimmed(path)?.parse().ok()
}

pub fn favg(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

pub fn fmin(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::INFINITY, f64::min)
}

pub fn fmax(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}
