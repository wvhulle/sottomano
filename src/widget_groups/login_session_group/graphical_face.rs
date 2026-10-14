use crate::{
    data::source::{hostname::entry, logind::session_facts::SessionIdentity},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
    },
};

pub struct GraphicalFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl BarWidget for GraphicalFace<'_> {
    fn name(&self) -> &'static str {
        "Graphical session"
    }

    fn icon(&self) -> Glyph {
        Phosphor::AppWindow(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.identity.desktop.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary(
            [entry("Desktop", self.identity.desktop.clone())]
                .into_iter()
                .flatten(),
        )
        .titled("Graphical session")])
    }
}
