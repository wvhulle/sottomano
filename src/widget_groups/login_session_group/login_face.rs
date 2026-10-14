use crate::{
    data::source::{hostname::entry, logind::session_facts::SessionIdentity},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
    },
};

pub struct LoginFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl BarWidget for LoginFace<'_> {
    fn name(&self) -> &'static str {
        "Login session"
    }

    fn icon(&self) -> Glyph {
        Phosphor::SignIn(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.identity.login_manager.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary(
            [
                Some(KeyValue::new(
                    "Login manager",
                    self.identity.login_manager.clone(),
                )),
                entry("Seat", self.identity.seat.clone()),
                Some(KeyValue::new(
                    "Session type",
                    self.identity.session_type.clone(),
                )),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Logged in through")])
    }
}
