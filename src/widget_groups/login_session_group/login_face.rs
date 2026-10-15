use crate::{
    data::source::{hostname::entry, logind::session_facts::SessionIdentity},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

pub struct LoginFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl BarWidget for LoginFace<'_> {
    fn name(&self) -> &'static str {
        "Login"
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
                    "Logged in through",
                    self.identity.login_manager.clone(),
                )),
                entry("Remote user", self.identity.remote_user.clone()),
                entry("Remote host", self.identity.remote_host.clone()),
                Some(KeyValue::new(
                    "Wiki",
                    Cell::Link {
                        label: "Display manager".to_owned(),
                        url: "https://wiki.archlinux.org/title/Display_manager".to_owned(),
                    },
                )),
            ]
            .into_iter()
            .flatten(),
        )
        .titled((
            "Login",
            "The login manager that checked your password and started this session.",
        ))])
    }
}
