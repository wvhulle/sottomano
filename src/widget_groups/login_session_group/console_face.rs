use crate::{
    data::source::hostname::entry,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

pub struct ConsoleFace<'a> {
    pub console: &'a str,
    pub virtual_terminal: Option<u32>,
}

impl BarWidget for ConsoleFace<'_> {
    fn name(&self) -> &'static str {
        "Console"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Terminal(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.console.to_owned())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary(
            [
                Some(KeyValue::new("Virtual console", self.console.to_owned())),
                entry(
                    "Switch with",
                    self.virtual_terminal.map(|vt| format!("Ctrl+Alt+F{vt}")),
                ),
                Some(KeyValue::new(
                    "Wiki",
                    Cell::Link {
                        label: "Linux console".to_owned(),
                        url: "https://wiki.archlinux.org/title/Linux_console".to_owned(),
                    },
                )),
            ]
            .into_iter()
            .flatten(),
        )
        .titled((
            "Console",
            "The text terminal this login started on; the compositor took it over.",
        ))])
    }
}
