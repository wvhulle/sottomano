use sottomano_desktop_sources::login_session_presence::logind_session_facts::{
    SessionIdentity, SessionInfo,
};

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct LoginFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl<'a> LoginFace<'a> {
    pub fn indicator(self) -> GroupIndicator<'a> {
        let label_text = self.identity.login_manager.clone();
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        GroupIndicator::glyph_only(
            "Login",
            Phosphor::SignIn(Weight::Regular).into(),
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
                "Login manager that let you in",
                "The login manager that checked your password and started this session.",
            ),
            dictionary_table([
                LabeledValue::new("Logged in through", self.identity.login_manager.clone()),
                LabeledValue::new(
                    "Wiki",
                    ValueCell::BrandedLink {
                        label: "Display manager".to_owned(),
                        url: "https://wiki.archlinux.org/title/Display_manager".to_owned(),
                        glyph: SimpleIcon::ArchLinux.into(),
                    },
                ),
            ]),
        )]
    }
}

pub fn session_info_sections(session_info: &SessionInfo) -> Vec<Section> {
    vec![
        Section::new(
            SectionTitle::captioned(
                "Login screen shown before this session",
                "Drew the password prompt in its own short session, closed once you logged in.",
            ),
            dictionary_table(
                session_info
                    .greeter
                    .clone()
                    .map(|greeter_name| LabeledValue::new("Greeter", greeter_name)),
            ),
        ),
        Section::new(
            SectionTitle::captioned(
                "Login session tracked by logind",
                "logind tracks this login and every process started from it.",
            ),
            dictionary_table(
                [
                    Some(LabeledValue::new("Session number", session_info.id.clone())),
                    Some(LabeledValue::new("State", session_info.state.clone())),
                    Some(LabeledValue::new(
                        "Logged in",
                        ValueCell::Time {
                            at: session_info.since,
                            format: LocalizedTimeFormat::DateAndTime,
                        },
                    )),
                    session_info.idle_since.map(|at| {
                        LabeledValue::new(
                            "Idle since",
                            ValueCell::Time {
                                at,
                                format: LocalizedTimeFormat::TimeOfDay,
                            },
                        )
                    }),
                ]
                .into_iter()
                .flatten(),
            ),
        ),
        Section::new(
            SectionTitle::captioned(
                "Processes belonging to this login",
                "Everything launched in this login lives in one systemd scope.",
            ),
            dictionary_table(
                [
                    session_info
                        .leader_name
                        .clone()
                        .map(|leader_name| LabeledValue::new("Started by", leader_name)),
                    Some(LabeledValue::new(
                        "Starter process id",
                        session_info.leader.to_string(),
                    )),
                    Some(LabeledValue::new(
                        "Processes kept in",
                        session_info.scope.clone(),
                    )),
                ]
                .into_iter()
                .flatten(),
            ),
        ),
    ]
    .into_iter()
    .filter(|section| !section.widget.is_empty())
    .collect()
}
