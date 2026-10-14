use iced::alignment::Horizontal;

use crate::{
    data::source::sysfs_sensors::cpu_tuning::CpuTuning,
    widgets::tooltip_body::{Cell, ColumnHeading, Section, TableRecord},
};

const HEADINGS: [ColumnHeading; 2] = [
    ColumnHeading::new("Value"),
    ColumnHeading::new("Meaning").align_x(Horizontal::Left),
];

fn explained_setting(key: &str, value: String, meaning: &str) -> TableRecord<2> {
    TableRecord::new(
        key,
        [Some(value.into()), Some(Cell::Hint(meaning.to_owned()))],
    )
}

impl CpuTuning {
    pub fn sections(&self) -> Vec<Section> {
        let driver = match &self.driver_mode {
            Some(mode) => format!("{} ({mode})", self.driver),
            None => self.driver.clone(),
        };
        let cpu = Section::column_table(
            HEADINGS,
            [
                Some(explained_setting(
                    "Frequency driver",
                    driver,
                    "kernel module that sets CPU clock speeds",
                )),
                self.governor.clone().map(|governor| {
                    explained_setting(
                        "Governor",
                        governor,
                        "policy the driver follows; set by the power profile",
                    )
                }),
                self.energy_preference.clone().map(|preference| {
                    explained_setting(
                        "Energy preference",
                        preference,
                        "hint to the CPU: favour speed or saving power",
                    )
                }),
                (!self.available_energy_preferences.is_empty()).then(|| {
                    explained_setting(
                        "Accepted preferences",
                        self.available_energy_preferences.join(", "),
                        "energy preferences the current governor allows",
                    )
                }),
                self.boost.map(|boost| {
                    explained_setting(
                        "Boost",
                        if boost { "on" } else { "off" }.to_owned(),
                        "short bursts above the base clock",
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("CPU");
        let firmware = Section::column_table(
            HEADINGS,
            [
                self.platform_profile.clone().map(|profile| {
                    explained_setting(
                        "Platform profile",
                        profile,
                        "fan and power limits chosen by the firmware",
                    )
                }),
                (!self.platform_profile_choices.is_empty()).then(|| {
                    explained_setting(
                        "Offered profiles",
                        self.platform_profile_choices.join(", "),
                        "platform profiles this machine supports",
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Firmware");
        vec![cpu, firmware]
    }
}
