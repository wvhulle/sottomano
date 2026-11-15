use sottomano_desktop_sources::login_session_presence::logind_session_facts::{
    SessionIdentity, SessionInfo,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{ReadingSections, Section, SectionTitle, dictionary_table},
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
    pub fn reading_row(self) -> ReadingRow<'a> {
        let label_text = self.identity.login_manager.clone();
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        ReadingRow {
            readout: RowReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Login",
                Phosphor::SignIn(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |clock| self.hover_card(clock)),
            )
        }
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
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

impl ReadingSections for SessionInfo {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![
            Section::new(
                SectionTitle::captioned(
                    "Login screen shown before this session",
                    "Drew the password prompt in its own short session, closed once you logged in.",
                ),
                dictionary_table(
                    self.greeter
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
                        Some(LabeledValue::new("Session number", self.id.clone())),
                        Some(LabeledValue::new("State", self.state.clone())),
                        Some(LabeledValue::new(
                            "Logged in",
                            ValueCell::Time {
                                at: self.since,
                                format: LocalizedTimeFormat::DateAndTime,
                            },
                        )),
                        self.idle_since.map(|at| {
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
                        self.leader_name
                            .clone()
                            .map(|leader_name| LabeledValue::new("Started by", leader_name)),
                        Some(LabeledValue::new(
                            "Starter process id",
                            self.leader.to_string(),
                        )),
                        Some(LabeledValue::new("Processes kept in", self.scope.clone())),
                    ]
                    .into_iter()
                    .flatten(),
                ),
            ),
        ]
        .into_iter()
        .filter(|section| !section.widget().is_empty())
        .collect()
    }
}
