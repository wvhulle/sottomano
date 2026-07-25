//! Display backlight level from sysfs, written back via `brightnessctl`.
//!
//! odin drives brightness through the `clightd`/`clight` D-Bus daemons (plus
//! `wl-gammarelay` for external monitors and `umbra` for dark-mode); those are
//! a bespoke setup. Here the core backlight level is read straight from
//! `/sys/class/backlight/<dev>/` and set with `brightnessctl`, which needs no
//! daemon. Off the bar when the machine has no backlight device (a desktop).

use std::{fs, path::PathBuf, time::Duration};

use crate::{
    data_sources::{
        BarWidget, Source, TooltipBody, Unit, Value,
        command::run,
        row,
    },
    widgets::icon::Icon,
};

#[derive(Debug, Clone)]
pub struct Backlight {
    pub percent: i64,
}

/// The first `/sys/class/backlight/*` device, sorted for a stable choice.
fn device() -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir("/sys/class/backlight")
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    entries.into_iter().next()
}

fn read_i64(path: PathBuf) -> Option<i64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

impl BarWidget for Backlight {
    fn icon(&self) -> Icon {
        Icon::Lightbulb
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![row("Brightness", format!("{}%", self.percent))])
    }
}

impl Source for Backlight {
    const NAME: &'static str = "backlight";
    const INTERVAL: Duration = Duration::from_secs(2);

    fn poll() -> Option<Self> {
        let device = device()?;
        let max = read_i64(device.join("max_brightness")).filter(|m| *m > 0)?;
        // `actual_brightness` reflects the hardware; fall back to the request.
        let current = read_i64(device.join("actual_brightness"))
            .or_else(|| read_i64(device.join("brightness")))?;
        Some(Backlight {
            percent: (current * 100 + max / 2) / max,
        })
    }
}

/// Set the backlight to `percent` (clamped 0..=100) via `brightnessctl`.
pub fn set_percent(percent: i64) -> bool {
    let clamped = percent.clamp(0, 100);
    run(
        "brightnessctl",
        &["-c", "backlight", "set", &format!("{clamped}%")],
    )
}
