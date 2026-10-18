use vetrata_sources::logind::session_facts::{SessionIdentity, SessionInfo};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::LocalizedTimeFormat,
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody, entry},
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
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Logged in through", self.identity.login_manager.clone()),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Display manager".to_owned(),
                    url: "https://wiki.archlinux.org/title/Display_manager".to_owned(),
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
    fn sections(&self) -> Vec<Section> {
        vec![
            Section::dictionary(entry("Greeter", self.greeter.clone())).titled((
                "Login screen shown before this session",
                "Drew the password prompt in its own short session, closed once you logged in.",
            )),
            Section::dictionary(
                [
                    Some(KeyValue::new("Session number", self.id.clone())),
                    Some(KeyValue::new("State", self.state.clone())),
                    Some(KeyValue::new(
                        "Logged in",
                        Cell::Time {
                            at: self.since,
                            format: LocalizedTimeFormat::DateAndTime,
                        },
                    )),
                    self.idle_since.map(|at| {
                        KeyValue::new(
                            "Idle since",
                            Cell::Time {
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
                    entry("Started by", self.leader_name.clone()),
                    Some(KeyValue::new("Starter process id", self.leader.to_string())),
                    Some(KeyValue::new("Processes kept in", self.scope.clone())),
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
