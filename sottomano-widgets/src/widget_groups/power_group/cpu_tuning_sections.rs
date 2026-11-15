use iced::alignment::Horizontal;
use sottomano_desktop_sources::battery_power_profiles::cpu_tuning_platform_profile::CpuTuning;

use crate::{
    hover_cards::hover_card_body::{Section, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        value_cells::ValueCell,
    },
};

const HEADINGS: [ColumnHeading; 2] = [
    ColumnHeading::titled("Value"),
    ColumnHeading::titled("Meaning").align_x(Horizontal::Left),
];

fn explained_setting(key: &str, value: String, meaning: &str) -> TableRecord<2> {
    TableRecord::with_cells(key, [value.into(), ValueCell::Hint(meaning.to_owned())])
}

pub fn cpu_tuning_sections(cpu_tuning: &CpuTuning) -> Vec<Section> {
    let driver = match &cpu_tuning.driver_mode {
        Some(mode) => format!("{} ({mode})", cpu_tuning.driver),
        None => cpu_tuning.driver.clone(),
    };
    let cpu = Section::new(
        "CPU",
        headed_table(
            HEADINGS,
            [
                Some(explained_setting(
                    "Frequency driver",
                    driver,
                    "kernel module that sets CPU clock speeds",
                )),
                cpu_tuning.governor.clone().map(|governor| {
                    explained_setting(
                        "Governor",
                        governor,
                        "policy the driver follows; set by the power profile",
                    )
                }),
                cpu_tuning.energy_preference.clone().map(|preference| {
                    explained_setting(
                        "Energy preference",
                        preference,
                        "hint to the CPU: favour speed or saving power",
                    )
                }),
                (!cpu_tuning.available_energy_preferences.is_empty()).then(|| {
                    explained_setting(
                        "Accepted preferences",
                        cpu_tuning.available_energy_preferences.join(", "),
                        "energy preferences the current governor allows",
                    )
                }),
                cpu_tuning.boost.map(|boost| {
                    explained_setting(
                        "Boost",
                        if boost { "on" } else { "off" }.to_owned(),
                        "short bursts above the base clock",
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        ),
    );
    let firmware = Section::new(
        "Firmware",
        headed_table(
            HEADINGS,
            [
                cpu_tuning.platform_profile.clone().map(|profile| {
                    explained_setting(
                        "Platform profile",
                        profile,
                        "fan and power limits chosen by the firmware",
                    )
                }),
                (!cpu_tuning.platform_profile_choices.is_empty()).then(|| {
                    explained_setting(
                        "Offered profiles",
                        cpu_tuning.platform_profile_choices.join(", "),
                        "platform profiles this machine supports",
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        ),
    );
    vec![cpu, firmware]
}
