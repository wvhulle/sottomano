use std::{collections::BTreeMap, path::Path};

use super::{ManagerProxy, UnitRow};
use crate::systemd_units_journal::unit_active_state::UnitActivity;

const SERVICE_SUFFIX: &str = ".service";
const LOADED_STATE: &str = "loaded";
const MASKED_FILE_STATE: &str = "masked";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TemplateInstanceUnit {
    pub(crate) unit_name: String,
    pub(crate) instance_name: String,
    pub(crate) activity: UnitActivity,
}

pub(crate) fn service_instance_name<'unit>(
    unit_name: &'unit str,
    template_prefixes: &[&str],
) -> Option<&'unit str> {
    let unit_stem = unit_name.strip_suffix(SERVICE_SUFFIX)?;
    template_prefixes.iter().find_map(|template_prefix| {
        unit_stem
            .strip_prefix(template_prefix)?
            .strip_prefix('@')
            .filter(|instance_name| !instance_name.is_empty())
    })
}

struct LoadedUnitActivity {
    unit_name: String,
    load_state: String,
    activity: UnitActivity,
}

impl From<UnitRow> for LoadedUnitActivity {
    fn from(unit_row: UnitRow) -> Self {
        Self {
            activity: UnitActivity::from(unit_row.active_state.as_str()),
            unit_name: unit_row.name,
            load_state: unit_row.load_state,
        }
    }
}

struct InstalledUnitFile {
    unit_name: String,
    file_state: String,
}

fn merged_template_instances(
    template_prefixes: &[&str],
    installed_files: impl IntoIterator<Item = InstalledUnitFile>,
    loaded_units: impl IntoIterator<Item = LoadedUnitActivity>,
) -> Vec<TemplateInstanceUnit> {
    let mut activity_by_unit: BTreeMap<String, UnitActivity> = installed_files
        .into_iter()
        .filter(|installed_file| installed_file.file_state != MASKED_FILE_STATE)
        .map(|installed_file| (installed_file.unit_name, UnitActivity::Stopped))
        .collect();
    activity_by_unit.extend(
        loaded_units
            .into_iter()
            .filter(|loaded_unit| {
                loaded_unit.load_state == LOADED_STATE
                    || loaded_unit.activity != UnitActivity::Stopped
            })
            .map(|loaded_unit| (loaded_unit.unit_name, loaded_unit.activity)),
    );
    activity_by_unit
        .into_iter()
        .filter_map(|(unit_name, activity)| {
            let instance_name = service_instance_name(&unit_name, template_prefixes)?.to_owned();
            Some(TemplateInstanceUnit {
                unit_name,
                instance_name,
                activity,
            })
        })
        .collect()
}

pub(crate) async fn listed_template_instances(
    connection: &zbus::Connection,
    template_prefixes: &[&str],
) -> zbus::Result<Vec<TemplateInstanceUnit>> {
    let manager = ManagerProxy::new(connection).await?;
    let unit_patterns: Vec<String> = template_prefixes
        .iter()
        .map(|template_prefix| format!("{template_prefix}@*{SERVICE_SUFFIX}"))
        .collect();
    let pattern_slices: Vec<&str> = unit_patterns.iter().map(String::as_str).collect();
    let installed_files = manager
        .list_unit_files_by_patterns(&[], &pattern_slices)
        .await?
        .into_iter()
        .filter_map(|(unit_file_path, file_state)| {
            let unit_name = Path::new(&unit_file_path).file_name()?.to_str()?.to_owned();
            Some(InstalledUnitFile {
                unit_name,
                file_state,
            })
        });
    let loaded_units = manager
        .list_units_by_patterns(&[], &pattern_slices)
        .await?
        .into_iter()
        .map(LoadedUnitActivity::from);
    Ok(merged_template_instances(
        template_prefixes,
        installed_files,
        loaded_units,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTAINER_TEMPLATES: [&str; 2] = ["container", "systemd-nspawn"];

    fn installed(unit_name: &str, file_state: &str) -> InstalledUnitFile {
        InstalledUnitFile {
            unit_name: unit_name.to_owned(),
            file_state: file_state.to_owned(),
        }
    }

    fn loaded(unit_name: &str, load_state: &str, activity: UnitActivity) -> LoadedUnitActivity {
        LoadedUnitActivity {
            unit_name: unit_name.to_owned(),
            load_state: load_state.to_owned(),
            activity,
        }
    }

    #[test]
    fn instance_names_come_from_either_template() {
        assert_eq!(
            service_instance_name("container@arr-proton.service", &CONTAINER_TEMPLATES),
            Some("arr-proton")
        );
        assert_eq!(
            service_instance_name("systemd-nspawn@debian.service", &CONTAINER_TEMPLATES),
            Some("debian")
        );
        assert_eq!(
            service_instance_name("container@.service", &CONTAINER_TEMPLATES),
            None
        );
        assert_eq!(
            service_instance_name("container@arr-proton.scope", &CONTAINER_TEMPLATES),
            None
        );
        assert_eq!(
            service_instance_name("containerd.service", &CONTAINER_TEMPLATES),
            None
        );
    }

    #[test]
    fn installed_but_unloaded_instances_read_as_stopped() {
        let instances = merged_template_instances(
            &CONTAINER_TEMPLATES,
            [
                installed("container@.service", "linked"),
                installed("container@arr-proton.service", "enabled"),
                installed("container@backup.service", "enabled"),
                installed("container@retired.service", "masked"),
            ],
            [loaded(
                "container@arr-proton.service",
                "loaded",
                UnitActivity::Running,
            )],
        );
        let activities: Vec<(&str, UnitActivity)> = instances
            .iter()
            .map(|instance| (instance.instance_name.as_str(), instance.activity))
            .collect();
        assert_eq!(
            activities,
            [
                ("arr-proton", UnitActivity::Running),
                ("backup", UnitActivity::Stopped),
            ]
        );
    }

    #[test]
    fn a_vanished_unit_file_is_dropped_once_its_unit_stops() {
        let instances = merged_template_instances(
            &CONTAINER_TEMPLATES,
            [],
            [
                loaded(
                    "container@removed.service",
                    "not-found",
                    UnitActivity::Stopped,
                ),
                loaded(
                    "container@draining.service",
                    "not-found",
                    UnitActivity::Stopping,
                ),
            ],
        );
        let instance_names: Vec<&str> = instances
            .iter()
            .map(|instance| instance.instance_name.as_str())
            .collect();
        assert_eq!(instance_names, ["draining"]);
    }
}
