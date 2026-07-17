//! sysfs / procfs data sources, ported in-process from odin's `battery.nu`, `cpu.nu`,
//! `cpu-clock.nu`, `memory.nu`, `temperature.nu`, `power-consumption.nu`. All reads are
//! direct `std::fs` on `/proc` and `/sys` — no subprocess, no `sensors`/`nu`.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use super::{BarWidget, Level, Row, Source, Unit, Value, row};

// ── Battery (battery.nu) ────────────────────────────────────────────────────

/// Power-source charge state, from `/sys/class/power_supply/*/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl ChargeStatus {
    fn parse(s: &str) -> Self {
        match s {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unknown => "Unknown",
        }
    }

    /// odin treats Full as "on AC / charged" — same logo as charging.
    fn charging(self) -> bool {
        matches!(self, Self::Charging | Self::Full)
    }
}

/// Battery charge level + power-source state (`battery.nu`).
#[derive(Debug, Clone)]
pub struct Battery {
    pub capacity: u8,
    pub status: ChargeStatus,
}

impl BarWidget for Battery {
    /// Two-state logo for the power source; the level is shown by the gauge.
    fn icon(&self) -> &'static str {
        if self.status.charging() {
            "plug-circle-bolt"
        } else {
            "car-battery"
        }
    }

    fn level(&self) -> Level {
        match self.capacity {
            0..=15 => Level::Critical,
            16..=30 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.capacity as i64,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        vec![
            row("Battery", format!("{}%", self.capacity)),
            row("Status", self.status.as_str()),
        ]
    }
}

impl Source for Battery {
    const NAME: &'static str = "battery";

    fn poll() -> Option<Self> {
        let dir = first_power_supply("BAT")?;
        let capacity = read_trimmed(dir.join("capacity"))?.parse().ok()?;
        let status = ChargeStatus::parse(&read_trimmed(dir.join("status"))?);
        Some(Battery { capacity, status })
    }
}

// ── Power draw (power-consumption.nu) ────────────────────────────────────────

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

/// Live system power draw in watts (`power-consumption.nu`), from `BAT0/power_now`.
#[derive(Debug, Clone)]
pub struct PowerDraw {
    pub watts: f64,
    pub status: ChargeStatus,
}

impl BarWidget for PowerDraw {
    fn icon(&self) -> &'static str {
        match self.status {
            ChargeStatus::Discharging | ChargeStatus::Unknown => "plug-circle-minus",
            _ => "plug-circle-plus",
        }
    }

    fn level(&self) -> Level {
        if self.status != ChargeStatus::Discharging {
            Level::Ok
        } else if self.watts > POWER_CRITICAL_W {
            Level::Critical
        } else if self.watts > POWER_WARN_W {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        // Map 0..GAUGE_MAX watts onto 0..100.
        let pct = (self.watts / POWER_GAUGE_MAX_W * 100.0).clamp(0.0, 100.0);
        Some(Value::Gauge {
            percent: pct.round() as i64,
            unit: Unit::Watt,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        let w = format!("{:.1} W", self.watts);
        match self.status {
            ChargeStatus::Charging => vec![row("Rate", w)],
            ChargeStatus::Full | ChargeStatus::NotCharging => vec![row("Power", "AC")],
            _ => vec![row("Draw", w)],
        }
    }
}

impl Source for PowerDraw {
    const NAME: &'static str = "power-consumption";

    fn poll() -> Option<Self> {
        let dir = Path::new("/sys/class/power_supply/BAT0");
        let uw: f64 = read_trimmed(dir.join("power_now"))?.parse().ok()?;
        let status = read_trimmed(dir.join("status"))
            .map(|s| ChargeStatus::parse(&s))
            .unwrap_or(ChargeStatus::Unknown);
        Some(PowerDraw {
            watts: uw / 1_000_000.0,
            status,
        })
    }
}

// ── CPU usage (cpu.nu) ───────────────────────────────────────────────────────

/// CPU busy percent over a 200 ms `/proc/stat` delta, plus per-core GHz (`cpu.nu`).
#[derive(Debug, Clone)]
pub struct CpuUsage {
    pub percent: i64,
    pub freqs_ghz: Vec<f64>,
}

impl BarWidget for CpuUsage {
    fn icon(&self) -> &'static str {
        "microchip"
    }

    fn level(&self) -> Level {
        match self.percent {
            p if p > 90 => Level::Critical,
            p if p > 70 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        let mut rows = vec![row("CPU", format!("{}%", self.percent))];
        if self.freqs_ghz.is_empty() {
            rows.push(row("Frequency", "unavailable"));
        } else {
            rows.push(row("Frequency min", ghz(fmin(&self.freqs_ghz))));
            rows.push(row("Frequency avg", ghz(favg(&self.freqs_ghz))));
            rows.push(row("Frequency max", ghz(fmax(&self.freqs_ghz))));
        }
        rows
    }
}

impl Source for CpuUsage {
    const NAME: &'static str = "cpu";

    fn poll() -> Option<Self> {
        let s1 = cpu_stat()?;
        thread::sleep(Duration::from_millis(200));
        let s2 = cpu_stat()?;

        let total: i64 = s1.iter().zip(&s2).map(|(a, b)| b - a).sum();
        let idle = s2.get(3).copied().unwrap_or(0) - s1.get(3).copied().unwrap_or(0);
        let percent = if total > 0 {
            (((total - idle) as f64 * 100.0) / total as f64).round() as i64
        } else {
            0
        };
        Some(CpuUsage {
            percent,
            freqs_ghz: core_ghz(),
        })
    }
}

/// The aggregate `cpu` line of `/proc/stat`, as its jiffy counters (idle at index 3).
fn cpu_stat() -> Option<Vec<i64>> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    let first = content.lines().next()?;
    Some(first.split_whitespace().skip(1).filter_map(|t| t.parse().ok()).collect())
}

// ── CPU clock (cpu-clock.nu) ─────────────────────────────────────────────────

/// One core's current frequency.
#[derive(Debug, Clone, Copy)]
pub struct CoreFreq {
    pub core: u32,
    pub ghz: f64,
}

/// Per-core CPU clock speeds (`cpu-clock.nu`). Bar label is the average GHz.
#[derive(Debug, Clone)]
pub struct CpuClock {
    pub cores: Vec<CoreFreq>,
}

impl CpuClock {
    fn avg(&self) -> f64 {
        favg(&self.cores.iter().map(|c| c.ghz).collect::<Vec<_>>())
    }
}

impl BarWidget for CpuClock {
    fn icon(&self) -> &'static str {
        "gauge-high"
    }

    fn label(&self) -> Option<String> {
        Some(ghz(self.avg()))
    }

    fn tooltip(&self) -> Vec<Row> {
        let ghzs: Vec<f64> = self.cores.iter().map(|c| c.ghz).collect();
        let mut rows = vec![
            row("Average", ghz(favg(&ghzs))),
            row("Min", ghz(fmin(&ghzs))),
            row("Max", ghz(fmax(&ghzs))),
        ];
        rows.extend(
            self.cores
                .iter()
                .map(|c| row(&format!("Core {}", c.core), ghz(c.ghz))),
        );
        rows
    }
}

impl Source for CpuClock {
    const NAME: &'static str = "cpu-clock";

    fn poll() -> Option<Self> {
        let cores = core_freqs();
        (!cores.is_empty()).then_some(CpuClock { cores })
    }
}

// ── Memory (memory.nu) ───────────────────────────────────────────────────────

const KIB_PER_GIB: f64 = 1_048_576.0;

/// RAM usage from `/proc/meminfo` (`memory.nu`).
#[derive(Debug, Clone)]
pub struct Memory {
    pub used_kb: i64,
    pub total_kb: i64,
}

impl Memory {
    fn percent(&self) -> i64 {
        ((self.used_kb as f64 * 100.0) / self.total_kb as f64).round() as i64
    }
}

impl BarWidget for Memory {
    fn icon(&self) -> &'static str {
        "memory"
    }

    fn level(&self) -> Level {
        match self.percent() {
            p if p > 90 => Level::Critical,
            p if p > 75 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent(),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        vec![
            row("Memory", format!("{}%", self.percent())),
            row("Used", format!("{:.1} GiB", self.used_kb as f64 / KIB_PER_GIB)),
            row("Total", format!("{:.1} GiB", self.total_kb as f64 / KIB_PER_GIB)),
        ]
    }
}

impl Source for Memory {
    const NAME: &'static str = "memory";

    fn poll() -> Option<Self> {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let total_kb = meminfo_kb(&info, "MemTotal")?;
        let available_kb = meminfo_kb(&info, "MemAvailable")?;
        Some(Memory {
            used_kb: total_kb - available_kb,
            total_kb,
        })
    }
}

/// The kB value of a `/proc/meminfo` line, e.g. `MemTotal:  32768000 kB`.
fn meminfo_kb(info: &str, key: &str) -> Option<i64> {
    info.lines().find_map(|line| {
        let rest = line.strip_prefix(key)?.strip_prefix(':')?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

// ── Temperature (temperature.nu) ─────────────────────────────────────────────

const CPU_CHIP_PREFIXES: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const TEMP_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

/// One per-core temperature sample.
#[derive(Debug, Clone)]
pub struct TempReading {
    pub chip: String,
    pub celsius: f64,
}

/// CPU temperature across all per-core sensors (`temperature.nu`), read directly from
/// hwmon rather than via `sensors`. Chip names are the raw hwmon `name` (e.g. `k10temp`),
/// not `sensors`' bus-qualified form (`k10temp-pci-00c3`) — a minor tooltip deviation.
#[derive(Debug, Clone)]
pub struct Temperature {
    pub readings: Vec<TempReading>,
}

impl Temperature {
    fn temps(&self) -> Vec<f64> {
        self.readings.iter().map(|r| r.celsius).collect()
    }
}

impl BarWidget for Temperature {
    fn icon(&self) -> &'static str {
        "temperature-half"
    }

    fn level(&self) -> Level {
        let avg = favg(&self.temps());
        if avg > 85.0 {
            Level::Critical
        } else if avg > 70.0 {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        // Gauge caps at 100 (°C read as a percent, odin's `[avg 100] | math min`).
        let avg = favg(&self.temps()).round() as i64;
        Some(Value::Gauge {
            percent: avg.min(100),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        let temps = self.temps();
        let mut chips: Vec<&str> = self.readings.iter().map(|r| r.chip.as_str()).collect();
        chips.dedup();
        vec![
            row("Chips", chips.join(", ")),
            row("Samples", self.readings.len().to_string()),
            row("Min", format!("{}°C", fmin(&temps).round() as i64)),
            row("Avg", format!("{}°C", favg(&temps).round() as i64)),
            row("Max", format!("{}°C", fmax(&temps).round() as i64)),
        ]
    }
}

impl Source for Temperature {
    const NAME: &'static str = "temperature";

    fn poll() -> Option<Self> {
        let readings = cpu_temp_readings();
        (!readings.is_empty()).then_some(Temperature { readings })
    }
}

/// Gather per-core temps from every CPU hwmon chip.
fn cpu_temp_readings() -> Vec<TempReading> {
    let mut chips = list_dir("/sys/class/hwmon");
    chips.sort();
    chips
        .iter()
        .filter_map(|chip| {
            let name = read_trimmed(chip.join("name"))?;
            let is_coretemp = name.starts_with("coretemp");
            CPU_CHIP_PREFIXES
                .iter()
                .any(|p| name.starts_with(p))
                .then(|| chip_readings(chip, &name, is_coretemp))
        })
        .flatten()
        .collect()
}

/// The per-core readings of one chip. Core labels are `Core N` (Intel) or `Tccd*`
/// (AMD); with none, fall back to the first present of Tdie/Tctl/Package id 0.
fn chip_readings(chip: &Path, name: &str, is_coretemp: bool) -> Vec<TempReading> {
    let labels: Vec<(u32, String)> = temp_labels(chip);
    let is_core = |label: &str| {
        if is_coretemp {
            label.starts_with("Core ")
        } else {
            label.starts_with("Tccd")
        }
    };

    let indices: Vec<u32> = if labels.iter().any(|(_, l)| is_core(l)) {
        labels.iter().filter(|(_, l)| is_core(l)).map(|(i, _)| *i).collect()
    } else {
        TEMP_FALLBACK_LABELS
            .iter()
            .find_map(|fb| labels.iter().find(|(_, l)| l == fb).map(|(i, _)| *i))
            .into_iter()
            .collect()
    };

    indices
        .iter()
        .filter_map(|i| read_i64(chip.join(format!("temp{i}_input"))))
        .map(|milli| TempReading {
            chip: name.to_string(),
            celsius: milli as f64 / 1000.0,
        })
        .collect()
}

/// `(index, label)` for every `tempN_label` on a chip.
fn temp_labels(chip: &Path) -> Vec<(u32, String)> {
    list_dir(chip)
        .iter()
        .filter_map(|p| {
            let stem = p.file_name()?.to_str()?;
            let index: u32 = stem.strip_prefix("temp")?.strip_suffix("_label")?.parse().ok()?;
            Some((index, read_trimmed(p)?))
        })
        .collect()
}

// ── shared sysfs/procfs helpers ──────────────────────────────────────────────

/// Per-core current frequency in GHz, from cpufreq sysfs, sorted by core index.
fn core_freqs() -> Vec<CoreFreq> {
    let mut cores: Vec<CoreFreq> = list_dir("/sys/devices/system/cpu")
        .iter()
        .filter_map(|dir| {
            let name = dir.file_name()?.to_str()?;
            let core: u32 = cpu_core_index(name)?;
            let khz = read_i64(dir.join("cpufreq/scaling_cur_freq"))?;
            Some(CoreFreq {
                core,
                ghz: khz as f64 / 1_000_000.0,
            })
        })
        .collect();
    cores.sort_by_key(|c| c.core);
    cores
}

/// Just the GHz values (`cpu.nu`'s frequency summary doesn't need core indices).
fn core_ghz() -> Vec<f64> {
    core_freqs().iter().map(|c| c.ghz).collect()
}

/// `cpuN` → `N`, rejecting `cpufreq`/`cpuidle` (non-numeric suffix).
fn cpu_core_index(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("cpu")?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// First `/sys/class/power_supply/<prefix>*` entry, sorted (odin's `glob BAT* | first`).
fn first_power_supply(prefix: &str) -> Option<PathBuf> {
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

fn list_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect()
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    Some(std::fs::read_to_string(path).ok()?.trim().to_string())
}

fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    read_trimmed(path)?.parse().ok()
}

/// GHz to 2 decimals with the unit — odin's `math round --precision 2` + `" GHz"`.
fn ghz(value: f64) -> String {
    format!("{value:.2} GHz")
}

fn favg(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn fmin(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::INFINITY, f64::min)
}

fn fmax(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}
