use chrono::Locale;
use sottomano_desktop_sources::login_session_presence::logind_session_facts::SessionIdentity;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct AccountFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl GroupItemReading for AccountFace<'_> {
    fn name(&self) -> &'static str {
        "Account"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::User(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.identity.user.clone())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([
            LabeledValue::new("User", self.identity.user.clone()),
            LabeledValue::new("User id", self.identity.uid.to_string()),
        ])
        .titled((
            "Account this session runs as",
            "The user this session runs as; everything above runs with its permissions.",
        ))])
    }
}
