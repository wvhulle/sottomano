use crate::{
    data::source::logind::session_facts::SessionIdentity,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
    },
};

pub struct AccountFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl BarWidget for AccountFace<'_> {
    fn name(&self) -> &'static str {
        "Account"
    }

    fn icon(&self) -> Glyph {
        Phosphor::User(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.identity.user.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([self.identity.section()])
    }
}
