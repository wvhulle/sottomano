use sottomano_desktop_sources::login_session_presence::logind_session_facts::SessionIdentity;

use super::{LoginSessionGroup, identity_refresh};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub struct AccountIndicator<'view> {
    pub identity: &'view SessionIdentity,
}

impl<'view> AccountIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let label_text = self.identity.user.clone();
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        GroupIndicator::glyph_only(
            "Account",
            Phosphor::User(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Account this session runs as",
                "The user this session runs as; everything above runs with its permissions.",
            ),
            dictionary_table([
                LabeledValue::new("User", self.identity.user.clone()),
                LabeledValue::new("User id", self.identity.uid.to_string()),
            ]),
        )]
    }
}

impl LoginSessionGroup {
    pub(super) fn account_row<'view>(
        &'view self,
        identity: &'view SessionIdentity,
    ) -> GroupIndicator<'view> {
        AccountIndicator { identity }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.identity
                    .hover_card_with(AccountIndicator { identity }.hover_card())
            }))
            .refreshed_by(identity_refresh())
    }
}
