use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, Section, TooltipBody},
};

pub struct SeatFace<'a> {
    pub seat: &'a str,
}

impl BarWidget for SeatFace<'_> {
    fn name(&self) -> &'static str {
        "Seat"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.seat.to_owned())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Hardware seat", self.seat.to_owned()),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Multiseat".to_owned(),
                    url: "https://wiki.archlinux.org/title/Multiseat".to_owned(),
                },
            ),
        ])
        .titled(("Seat", "The screen, keyboard and mouse this login owns."))])
    }
}
