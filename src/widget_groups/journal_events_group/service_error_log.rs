use crate::{
    data::{
        severity::Severity,
        source::service_errors::{ServiceErrors, UnitErrors},
    },
    widget_groups::journal_events_group::MESSAGE_CAP,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Section, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

impl TableRow for UnitErrors {
    const HEADINGS: &'static [&'static str] = &["Latest", "Count", "Message"];

    fn key(&self) -> Cell {
        self.unit.name().into()
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Time {
                at: self.latest.into(),
                part: TimePart::TimeOfDay,
            },
            self.count.to_string().into(),
            self.latest_message
                .chars()
                .take(MESSAGE_CAP)
                .collect::<String>()
                .into(),
        ]
    }

    fn level(&self) -> Severity {
        self.severity
    }
}

impl BarWidget for ServiceErrors {
    fn icon(&self) -> Glyph {
        Phosphor::WarningCircle(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.by_unit_oldest_first.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.by_unit_oldest_first
            .iter()
            .map(|errors| errors.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::table(
            (
                "Service errors",
                "Journal entries at err or worse from .service units in the last hour, newest \
                 message per service.",
            ),
            &self.by_unit_oldest_first,
        )])
    }
}
