use crate::{
    data::source::logind::session_facts::SessionIdentity,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

pub struct CompositorFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl BarWidget for CompositorFace<'_> {
    fn name(&self) -> &'static str {
        "Compositor"
    }

    fn icon(&self) -> Glyph {
        Phosphor::AppWindow(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.identity.desktop.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Protocol", self.identity.session_type.clone()),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Wayland".to_owned(),
                    url: "https://wiki.archlinux.org/title/Wayland".to_owned(),
                },
            ),
        ])
        .titled((
            "Compositor drawing the desktop",
            "The compositor draws every window and routes input; apps connect to it.",
        ))])
    }
}
