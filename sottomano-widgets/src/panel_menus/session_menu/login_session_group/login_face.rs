use sottomano_desktop_sources::login_session_presence::logind_session_facts::{
    GreeterProgram, LeaderName, SessionIdentity, SessionIdleness, SessionInfo,
};

use crate::{
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct LoginFace<'view> {
    pub identity: &'view SessionIdentity,
}

impl<'view> LoginFace<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
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
    let mut login_entries = vec![
        LabeledValue::new("Session number", session_info.id.clone()),
        LabeledValue::new("State", session_info.state.clone()),
        LabeledValue::new(
            "Logged in",
            ValueCell::Time {
                at: session_info.since,
                format: LocalizedTimeFormat::DateAndTime,
            },
        ),
    ];
    if let SessionIdleness::IdleSince(idle_at) = session_info.idleness {
        login_entries.push(LabeledValue::new(
            "Idle since",
            ValueCell::Time {
                at: idle_at,
                format: LocalizedTimeFormat::TimeOfDay,
            },
        ));
    }
    let mut process_entries = Vec::new();
    if let LeaderName::Named(leader_name) = &session_info.leader_name {
        process_entries.push(LabeledValue::new("Started by", leader_name.clone()));
    }
    process_entries.push(LabeledValue::new(
        "Starter process id",
        session_info.leader.to_string(),
    ));
    process_entries.push(LabeledValue::new(
        "Processes kept in",
        session_info.scope.clone(),
    ));
    let mut sections = Vec::new();
    if let GreeterProgram::Identified(greeter_name) = &session_info.greeter {
        sections.push(Section::new(
            SectionTitle::captioned(
                "Login screen shown before this session",
                "Drew the password prompt in its own short session, closed once you logged in.",
            ),
            dictionary_table([LabeledValue::new("Greeter", greeter_name.clone())]),
        ));
    }
    sections.push(Section::new(
        SectionTitle::captioned(
            "Login session tracked by logind",
            "logind tracks this login and every process started from it.",
        ),
        dictionary_table(login_entries),
    ));
    sections.push(Section::new(
        SectionTitle::captioned(
            "Processes belonging to this login",
            "Everything launched in this login lives in one systemd scope.",
        ),
        dictionary_table(process_entries),
    ));
    sections
}
