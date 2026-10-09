use crate::{
    data::{
        severity::Severity,
        source::systemd_manager::boot_timeline::{
            BootPhase, BootPhases, RankedActivations, SystemStartup, UnitActivation, UserStartup,
        },
    },
    widget_groups::systemd_manager_group::elapsed_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, Section, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

impl TableRow for UnitActivation {
    const HEADINGS: &'static [&'static str] = &[];

    fn key(&self) -> Cell {
        self.name.clone().into()
    }

    fn cells(&self) -> Vec<Cell> {
        vec![elapsed_text(self.duration).into()]
    }

    fn level(&self) -> Severity {
        self.severity
    }
}

impl BootPhase {
    fn row(self) -> Row {
        Row::new(
            self.stage.name(),
            &[
                elapsed_text(self.duration).into(),
                Cell::Hint(self.stage.meaning().to_owned()),
            ],
            Severity::Normal,
        )
    }
}

impl BarWidget for BootPhases {
    fn icon(&self) -> Glyph {
        Phosphor::RocketLaunch(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(elapsed_text(self.total()))
    }

    fn tooltip(&self) -> TooltipBody {
        let phases: Vec<Row> = self.present().into_iter().map(BootPhase::row).collect();
        TooltipBody::sections(&[Section::untitled(&phases)])
    }
}

impl RankedActivations {
    fn slow_badge(&self) -> Option<u32> {
        u32::try_from(self.slow_count())
            .ok()
            .filter(|count| *count > 0)
    }

    fn slowness_state(&self) -> WidgetState {
        self.slowest_first
            .iter()
            .map(|activation| activation.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn table(&self, title: &str) -> TooltipBody {
        TooltipBody::sections(&[Section::table(
            (
                title,
                "Time from starting to active, for units up before startup finished. Devices and \
                 units under 0.1 s are left out; amber ones crossed slow_boot_unit_secs.",
            ),
            &self.slowest_first,
        )])
    }
}

impl BarWidget for SystemStartup {
    fn icon(&self) -> Glyph {
        Phosphor::Stack(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        self.slowest_units.slow_badge()
    }

    fn state(&self) -> WidgetState {
        self.slowest_units.slowness_state()
    }

    fn tooltip(&self) -> TooltipBody {
        self.slowest_units.table("Slowest system units at boot")
    }
}

impl BarWidget for UserStartup {
    fn icon(&self) -> Glyph {
        Phosphor::UserCircle(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(elapsed_text(self.duration))
    }

    fn badge(&self) -> Option<u32> {
        self.slowest_units.slow_badge()
    }

    fn state(&self) -> WidgetState {
        self.slowest_units.slowness_state()
    }

    fn tooltip(&self) -> TooltipBody {
        self.slowest_units.table("Slowest user units at login")
    }
}
