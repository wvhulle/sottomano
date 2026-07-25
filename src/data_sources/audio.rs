//! Default audio sink (speakers) and source (microphone) volume via `wpctl`
//! (PipeWire/WirePlumber). One generic source parameterised on direction:
//! `INPUT` selects the capture device (mic) over the playback device
//! (speakers), which flips the target node, the icon set, and the severity
//! thresholds (a quiet mic is bad; a loud sink is bad).
//!
//! odin drives these from a `pw-mon` stream for instant updates; here they are
//! polled once a second, with the displayed value nudged optimistically on a
//! scroll so the gauge tracks the wheel before the next poll lands.

use std::time::Duration;

use crate::{
    data_sources::{
        BarWidget, Level, Source, TooltipBody, Unit, Value,
        command::{exists, output, run},
        row,
    },
    widgets::icon::Icon,
};

/// The `wpctl` special-id for this direction's default node.
const fn node<const INPUT: bool>() -> &'static str {
    if INPUT {
        "@DEFAULT_AUDIO_SOURCE@"
    } else {
        "@DEFAULT_AUDIO_SINK@"
    }
}

#[derive(Debug, Clone)]
pub struct Audio<const INPUT: bool> {
    pub percent: i64,
    muted: bool,
    device: Option<String>,
    /// No default node for this direction (odin's error envelope): a dimmed,
    /// critical placeholder rather than an empty slot.
    errored: bool,
}

impl<const INPUT: bool> Audio<INPUT> {
    /// The default node's volume percent and mute state from `wpctl get-volume`
    /// (`Volume: 0.65` or `Volume: 0.65 [MUTED]`).
    fn read_volume() -> Option<(i64, bool)> {
        let line = output("wpctl", &["get-volume", node::<INPUT>()])?;
        let rest = line.trim().strip_prefix("Volume:")?;
        let muted = rest.contains("[MUTED]");
        let fraction: f64 = rest.replace("[MUTED]", "").trim().parse().ok()?;
        #[allow(clippy::cast_possible_truncation)]
        let percent = (fraction * 100.0).round() as i64;
        // odin treats a zero-volume node as muted too.
        Some((percent, muted || percent == 0))
    }

    /// A friendly device name from `wpctl inspect` (`node.nick`, else
    /// `node.description`), for the tooltip.
    fn read_device() -> Option<String> {
        let inspect = output("wpctl", &["inspect", node::<INPUT>()])?;
        let field = |key: &str| {
            inspect.lines().find_map(|line| {
                let line = line.trim().trim_start_matches("* ");
                let value = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
                Some(value.trim().trim_matches('"').to_string())
            })
        };
        field("node.nick").or_else(|| field("node.description"))
    }
}

impl<const INPUT: bool> BarWidget for Audio<INPUT> {
    fn icon(&self) -> Icon {
        if INPUT {
            if self.muted || self.errored {
                Icon::MicrophoneSlash
            } else {
                Icon::Microphone
            }
        } else if self.muted || self.errored {
            Icon::VolumeXmark
        } else if self.percent > 50 {
            Icon::VolumeHigh
        } else {
            Icon::VolumeLow
        }
    }

    fn disabled(&self) -> bool {
        self.muted || self.errored
    }

    fn level(&self) -> Level {
        if self.errored {
            return Level::Critical;
        }
        if INPUT {
            // A quiet mic is the severe end.
            match self.percent {
                ..15 => Level::Critical,
                15..50 => Level::Warning,
                _ => Level::Ok,
            }
        } else {
            // A loud sink is the severe end.
            match self.percent {
                95.. => Level::Critical,
                80..95 => Level::Warning,
                _ => Level::Ok,
            }
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let label = if INPUT { "Microphone" } else { "Volume" };
        if self.errored {
            let what = if INPUT { "source" } else { "sink" };
            return TooltipBody::Table(vec![row("State", format!("no default {what}"))]);
        }
        let mut rows = vec![
            row(label, format!("{}%", self.percent)),
            row("State", if self.muted { "muted" } else { "active" }),
        ];
        if let Some(device) = &self.device {
            rows.push(row("Device", device.clone()));
        }
        TooltipBody::Table(rows)
    }
}

impl<const INPUT: bool> Source for Audio<INPUT> {
    const NAME: &'static str = if INPUT { "audio-source" } else { "audio-sink" };
    const INTERVAL: Duration = Duration::from_secs(1);

    fn poll() -> Option<Self> {
        if !exists("wpctl") {
            return None;
        }
        let Some((percent, muted)) = Self::read_volume() else {
            return Some(Audio {
                percent: 0,
                muted: false,
                device: None,
                errored: true,
            });
        };
        Some(Audio {
            percent,
            muted,
            device: Self::read_device(),
            errored: false,
        })
    }
}

/// Set this direction's default-node volume to `percent` (clamped 0..=100).
pub fn set_volume<const INPUT: bool>(percent: i64) -> bool {
    let clamped = percent.clamp(0, 100);
    // Format as a `0.NN` fraction without a float cast (values are 0..=100).
    let fraction = format!("{}.{:02}", clamped / 100, clamped % 100);
    run("wpctl", &["set-volume", node::<INPUT>(), &fraction])
}

/// Toggle mute on this direction's default node.
pub fn toggle_mute<const INPUT: bool>() -> bool {
    run("wpctl", &["set-mute", node::<INPUT>(), "toggle"])
}
