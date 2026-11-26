use iced::alignment::Horizontal;
use sottomano_desktop_sources::battery_power_profiles::cpu_tuning_platform_profile::{
    BoostSetting, CpuTuning, KernelSetting,
};

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
        KernelSetting::Exposed(mode) => format!("{} ({mode})", cpu_tuning.driver),
        KernelSetting::NotExposed => cpu_tuning.driver.clone(),
    };
    let mut cpu_rows = vec![explained_setting(
        "Frequency driver",
        driver,
        "kernel module that sets CPU clock speeds",
    )];
    if let KernelSetting::Exposed(governor) = &cpu_tuning.governor {
        cpu_rows.push(explained_setting(
            "Governor",
            governor.clone(),
            "policy the driver follows; set by the power profile",
        ));
    }
    if let KernelSetting::Exposed(preference) = &cpu_tuning.energy_preference {
        cpu_rows.push(explained_setting(
            "Energy preference",
            preference.clone(),
            "hint to the CPU: favour speed or saving power",
        ));
    }
    if !cpu_tuning.available_energy_preferences.is_empty() {
        cpu_rows.push(explained_setting(
            "Accepted preferences",
            cpu_tuning.available_energy_preferences.join(", "),
            "energy preferences the current governor allows",
        ));
    }
    match cpu_tuning.boost {
        BoostSetting::On => cpu_rows.push(explained_setting(
            "Boost",
            "on".to_owned(),
            "short bursts above the base clock",
        )),
        BoostSetting::Off => cpu_rows.push(explained_setting(
            "Boost",
            "off".to_owned(),
            "short bursts above the base clock",
        )),
        BoostSetting::NotExposed => {}
    }
    let mut firmware_rows = Vec::new();
    if let KernelSetting::Exposed(profile) = &cpu_tuning.platform_profile {
        firmware_rows.push(explained_setting(
            "Platform profile",
            profile.clone(),
            "fan and power limits chosen by the firmware",
        ));
    }
    if !cpu_tuning.platform_profile_choices.is_empty() {
        firmware_rows.push(explained_setting(
            "Offered profiles",
            cpu_tuning.platform_profile_choices.join(", "),
            "platform profiles this machine supports",
        ));
    }
    vec![
        Section::new("CPU", headed_table(HEADINGS, cpu_rows)),
        Section::new("Firmware", headed_table(HEADINGS, firmware_rows)),
    ]
}
