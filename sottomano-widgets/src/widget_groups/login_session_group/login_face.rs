use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::login_session_presence::logind_session_facts::{
    SessionIdentity, SessionInfo,
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct LoginFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl GroupItemReading for LoginFace<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Login".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::SignIn(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.identity.login_manager.clone())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([
            LabeledValue::new("Logged in through", self.identity.login_manager.clone()),
            LabeledValue::new(
                "Wiki",
                ValueCell::BrandedLink {
                    label: "Display manager".to_owned(),
                    url: "https://wiki.archlinux.org/title/Display_manager".to_owned(),
                    glyph: SimpleIcon::ArchLinux.into(),
                },
            ),
        ])
        .titled((
            "Login manager that let you in",
            "The login manager that checked your password and started this session.",
        ))])
    }
}

impl ReadingSections for SessionInfo {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![
            Section::dictionary(
                self.greeter
                    .clone()
                    .map(|greeter_name| LabeledValue::new("Greeter", greeter_name)),
            )
            .titled((
                "Login screen shown before this session",
                "Drew the password prompt in its own short session, closed once you logged in.",
            )),
            Section::dictionary(
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
            )
            .titled((
                "Login session tracked by logind",
                "logind tracks this login and every process started from it.",
            )),
            Section::dictionary(
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
            )
            .titled((
                "Processes belonging to this login",
                "Everything launched in this login lives in one systemd scope.",
            )),
        ]
        .into_iter()
        .filter(|section| !section.body().is_empty())
        .collect()
    }
}
