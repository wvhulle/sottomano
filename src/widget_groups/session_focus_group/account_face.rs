use crate::{
    data::source::logind::session_facts::SessionIdentity,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
    },
};

#[derive(Default)]
pub struct AccountFace {
    identity: Option<SessionIdentity>,
}

impl AccountFace {
    pub fn adopt(&mut self, identity: Option<SessionIdentity>) {
        self.identity = identity;
    }

    pub fn identity(&self) -> Option<&SessionIdentity> {
        self.identity.as_ref()
    }
}

impl BarWidget for AccountFace {
    fn name(&self) -> &'static str {
        "Session"
    }

    fn icon(&self) -> Glyph {
        Phosphor::User(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.identity.as_ref().map(|identity| identity.user.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.identity.iter().map(SessionIdentity::section))
    }
}
