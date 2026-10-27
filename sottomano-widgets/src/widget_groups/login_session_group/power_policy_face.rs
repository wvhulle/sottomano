use iced::alignment::Horizontal;
use sottomano_sources::login_session_presence::logind_session_facts::{InhibitMode, PowerPolicy};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, ReadingSections, Section, TableRecord, TooltipBody,
        },
    },
};

pub struct PowerPolicyFace;

impl BarWidget for PowerPolicyFace {
    fn name(&self) -> &'static str {
        "Power policy"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Plug(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Decided by", "systemd-logind"),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Power management".to_owned(),
                    url: "https://wiki.archlinux.org/title/Power_management".to_owned(),
                    glyph: Some(SimpleIcon::ArchLinux.into()),
                },
            ),
        ])
        .titled((
            "Machine-wide power policy",
            "What the power key, lid and idle timeout do, for every login on this machine.",
        ))])
    }
}

fn yes_no(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

impl ReadingSections for PowerPolicy {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let idle_after = self
            .idle_action_after
            .filter(|_| self.idle_action != "ignore")
            .map(|after| KeyValue::new("Idle after", Cell::duration(after)));
        let sleep_tries = self
            .sleep_order
            .iter()
            .enumerate()
            .map(|(position, operation)| {
                KeyValue::new(format!("Sleep try {}", position + 1), operation.clone())
            });
        let locks_in_mode = |mode: InhibitMode| {
            self.inhibitor_locks
                .iter()
                .filter(move |lock| lock.mode == mode)
                .map(|lock| {
                    TableRecord::new(
                        lock.who.clone(),
                        [
                            Some(lock.operation.clone().into()),
                            lock.why.clone().map(Cell::from),
                        ],
                    )
                })
        };
        let lock_headings = [
            ColumnHeading::titled("Holds").align_x(Horizontal::Left),
            ColumnHeading::titled("Why").align_x(Horizontal::Left),
        ];
        let scheduled = self.scheduled_shutdown.as_ref().map(|shutdown| {
            let key = match shutdown.kind.as_str() {
                "poweroff" => "Power off scheduled",
                "reboot" => "Reboot scheduled",
                "halt" => "Halt scheduled",
                _ => "Scheduled shutdown",
            };
            match shutdown.at {
                Some(at) => KeyValue::new(
                    key,
                    Cell::Time {
                        at,
                        format: LocalizedTimeFormat::DateAndTime,
                    },
                ),
                None => KeyValue::new(key, shutdown.kind.clone()),
            }
        });
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Power key", self.power_key.clone())),
                    Some(KeyValue::new("Suspend key", self.suspend_key.clone())),
                    Some(KeyValue::new("Hibernate key", self.hibernate_key.clone())),
                    Some(KeyValue::new("Reboot key", self.reboot_key.clone())),
                    Some(KeyValue::new("Lid switch", self.lid_switch.clone())),
                    Some(KeyValue::new("Idle action", self.idle_action.clone())),
                    idle_after,
                ]
                .into_iter()
                .flatten()
                .chain(sleep_tries),
            )
            .titled((
                "What the keys and lid trigger",
                "What logind does when these are pressed or closed.",
            )),
            Section::dictionary(
                [
                    Some(KeyValue::new("Docked", yes_no(self.chassis.docked))),
                    Some(KeyValue::new("Lid closed", yes_no(self.chassis.lid_closed))),
                    Some(KeyValue::new(
                        "External power",
                        yes_no(self.chassis.on_external_power),
                    )),
                    Some(KeyValue::new("Sessions", Quantity::count(self.sessions))),
                    scheduled,
                    self.preparing_for_shutdown
                        .then(|| KeyValue::new("Preparing for shutdown", "yes")),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Current state of the machine",
                "Conditions that change which of these actions applies.",
            )),
            Section::column_table(lock_headings, locks_in_mode(InhibitMode::Delay)).titled((
                "Apps briefly holding off sleep or shutdown",
                "Each gets a few seconds to clean up first; the action still happens.",
            )),
            Section::column_table(lock_headings, locks_in_mode(InhibitMode::Block)).titled((
                "Apps that took over from logind",
                "logind skips its own action; the app handles or prevents it.",
            )),
        ]
        .into_iter()
        .filter(|section| !section.body().is_empty())
        .collect()
    }
}
