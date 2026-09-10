use bytesize::ByteSize;

use crate::{
    data::source::disk::Disk,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

const FREE_WARN_BYTES: u64 = 100_000_000_000;
const FREE_CRITICAL_BYTES: u64 = 2_000_000_000;

impl BarWidget for Disk {
    fn icon(&self) -> Glyph {
        Solid::HardDrive.into()
    }

    fn level(&self) -> Level {
        if self.free_bytes < FREE_CRITICAL_BYTES {
            Level::Critical
        } else if self.free_bytes < FREE_WARN_BYTES {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.used_pct(),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Mount", "/"),
            row("Used", format!("{}%", self.used_pct())),
            row("Free", bytes(self.free_bytes)),
            row("Total", bytes(self.total_bytes)),
        ])
    }
}

fn bytes(amount: u64) -> String {
    ByteSize::b(amount).display().iec().to_string()
}
