//! sysfs / procfs hardware sensor sources, ported in-process from odin's
//! `battery.nu`, `cpu.nu`, `cpu-clock.nu`, `memory.nu`, `temperature.nu`,
//! `power-consumption.nu`. All reads are direct `std::fs` on `/proc` and `/sys`
//! — no subprocess, no `sensors`/`nu`.

use std::path::{Path, PathBuf};

pub mod battery;
pub mod cpu_clock;
pub mod cpu_usage;
pub mod memory;
pub mod power;
pub mod temperature;

/// Per-core current frequency in GHz, from cpufreq sysfs, sorted by core index.
/// Shared by `cpu_clock` (per-core detail) and `cpu_usage` (the bar's average
/// summary).
pub(super) fn core_freqs() -> Vec<cpu_clock::CoreFreq> {
    let mut cores: Vec<cpu_clock::CoreFreq> = list_dir("/sys/devices/system/cpu")
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

/// Just the GHz values (`cpu.nu`'s frequency summary doesn't need core
/// indices).
pub(super) fn core_ghz() -> Vec<f64> {
    core_freqs().iter().map(|c| c.ghz).collect()
}

/// `cpuN` → `N`, rejecting `cpufreq`/`cpuidle` (non-numeric suffix).
fn cpu_core_index(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("cpu")?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// First `/sys/class/power_supply/<prefix>*` entry, sorted (odin's `glob BAT* |
/// first`).
pub(super) fn first_power_supply(prefix: &str) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = list_dir("/sys/class/power_supply")
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

pub(super) fn list_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect()
}

pub(super) fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    Some(std::fs::read_to_string(path).ok()?.trim().to_string())
}

pub(super) fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    read_trimmed(path)?.parse().ok()
}

/// GHz to 2 decimals with the unit — odin's `math round --precision 2` + `"
/// GHz"`.
pub(super) fn ghz(value: f64) -> String {
    format!("{value:.2} GHz")
}

pub(super) fn favg(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

pub(super) fn fmin(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::INFINITY, f64::min)
}

pub(super) fn fmax(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}
