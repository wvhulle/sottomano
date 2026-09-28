use bytesize::ByteSize;

use crate::{
    data::source::disk::Disk,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        phosphor,
        tooltip_body::{TooltipBody, row},
    },
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);

impl BarWidget for Disk {
    fn icon(&self) -> Glyph {
        phosphor::HardDrive::Regular.into()
    }

    fn level(&self) -> Level {
        if self.free < FREE_CRITICAL {
            Level::Critical
        } else if self.free < FREE_WARN {
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
            row("Free", self.free.display().iec().to_string()),
            row("Total", self.total.display().iec().to_string()),
        ])
    }
}
