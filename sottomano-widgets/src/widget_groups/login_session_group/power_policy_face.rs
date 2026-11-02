use iced::alignment::Horizontal;
use sottomano_sources::login_session_presence::logind_session_facts::{
    InhibitMode, InhibitorLock, PowerPolicy,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, ReadingSections, Section},
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct PowerPolicyFace;

impl GroupItemReading for PowerPolicyFace {
    fn name(&self) -> &'static str {
        "Power policy"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Plug(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([
            LabeledValue::new("Decided by", "systemd-logind"),
            LabeledValue::new(
                "Wiki",
                ValueCell::BrandedLink {
                    label: "Power management".to_owned(),
                    url: "https://wiki.archlinux.org/title/Power_management".to_owned(),
                    glyph: SimpleIcon::ArchLinux.into(),
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

impl IntoTableRecord<2> for &InhibitorLock {
    fn into_table_record(self) -> TableRecord<2> {
        TableRecord::new(
            self.who.clone(),
            [
                self.operation.clone().into(),
                self.why.clone().map_or(ValueCell::Blank, ValueCell::from),
            ],
        )
    }
}

impl ReadingSections for PowerPolicy {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let idle_after = self
            .idle_action_after
            .filter(|_| self.idle_action != "ignore")
            .map(|after| LabeledValue::new("Idle after", ValueCell::duration(after)));
        let sleep_tries = self
            .sleep_order
            .iter()
            .enumerate()
            .map(|(position, operation)| {
                LabeledValue::new(format!("Sleep try {}", position + 1), operation.clone())
            });
        let locks_in_mode = |mode: InhibitMode| {
            self.inhibitor_locks
                .iter()
                .filter(move |lock| lock.mode == mode)
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
                Some(at) => LabeledValue::new(
                    key,
                    ValueCell::Time {
                        at,
                        format: LocalizedTimeFormat::DateAndTime,
                    },
                ),
                None => LabeledValue::new(key, shutdown.kind.clone()),
            }
        });
        vec![
            Section::dictionary(
                [
                    Some(LabeledValue::new("Power key", self.power_key.clone())),
                    Some(LabeledValue::new("Suspend key", self.suspend_key.clone())),
                    Some(LabeledValue::new(
                        "Hibernate key",
                        self.hibernate_key.clone(),
                    )),
                    Some(LabeledValue::new("Reboot key", self.reboot_key.clone())),
                    Some(LabeledValue::new("Lid switch", self.lid_switch.clone())),
                    Some(LabeledValue::new("Idle action", self.idle_action.clone())),
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
                    Some(LabeledValue::new("Docked", yes_no(self.chassis.docked))),
                    Some(LabeledValue::new(
                        "Lid closed",
                        yes_no(self.chassis.lid_closed),
                    )),
                    Some(LabeledValue::new(
                        "External power",
                        yes_no(self.chassis.on_external_power),
                    )),
                    Some(LabeledValue::new(
                        "Sessions",
                        Quantity::count(self.sessions),
                    )),
                    scheduled,
                    self.preparing_for_shutdown
                        .then(|| LabeledValue::new("Preparing for shutdown", "yes")),
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
