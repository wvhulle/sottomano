use sottomano_desktop_sources::login_session_presence::kernel_sleep_obstacles::{
    InhibitedOperation, KernelHibernation, KernelSleepVerdict, LogindCapability,
    SecretMemoryHolder, SleepObstacles, SleepState,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub(in crate::indicator_groups::idle_pause_groups) fn sleep_state_sections(
    obstacles: &SleepObstacles,
) -> Vec<Section> {
    let state_that_runs = obstacles.sleep_state_that_runs();
    let rows = SleepState::ALL.into_iter().map(|state| {
        let answer = sleep_state_answer(obstacles, state, state_that_runs);
        let row = LabeledValue::new(state.title(), answer);
        match obstacles.sleep_state_capability(state) {
            LogindCapability::Allowed | LogindCapability::AfterAuthorisation => row,
            LogindCapability::NotPermitted | LogindCapability::Unsupported => {
                row.level(Severity::Warning)
            }
        }
    });
    vec![Section::new(
        SectionTitle::captioned(
            "What standby does now",
            "logind runs the first available state in its configured order.",
        ),
        dictionary_table(rows),
    )]
}

pub(super) fn sleep_state_answer(
    obstacles: &SleepObstacles,
    state: SleepState,
    state_that_runs: Option<SleepState>,
) -> String {
    if state_that_runs == Some(state) {
        return "will run".to_owned();
    }
    let sleep_lock_holders: Vec<&str> = obstacles
        .inhibitors_blocking(InhibitedOperation::Sleep)
        .map(|inhibitor| inhibitor.holder.as_str())
        .collect();
    match obstacles.sleep_state_capability(state) {
        LogindCapability::Allowed => "available".to_owned(),
        LogindCapability::AfterAuthorisation => "available after authorisation".to_owned(),
        _ if obstacles.kernel_verdict(state) == KernelSleepVerdict::Blocked => {
            kernel_refusal_text(&obstacles.kernel_hibernation)
        }
        _ if !sleep_lock_holders.is_empty() => {
            format!(
                "blocked by a sleep lock from {}",
                sleep_lock_holders.join(", ")
            )
        }
        LogindCapability::NotPermitted => "not permitted for this user".to_owned(),
        LogindCapability::Unsupported => unsupported_by_logind_text().to_owned(),
    }
}

pub(super) fn kernel_refusal_text(kernel_hibernation: &KernelHibernation) -> String {
    match kernel_hibernation {
        KernelHibernation::Allowed => unsupported_by_logind_text().to_owned(),
        KernelHibernation::DisabledByBootFlag(flag) => {
            format!("disabled by the kernel: booted with {flag}")
        }
        KernelHibernation::DisabledByLockdown(level) => {
            format!("disabled by the kernel: lockdown is {level}")
        }
        KernelHibernation::DisabledBySecretMemory(holders) => format!(
            "disabled by the kernel while {} holds secret memory",
            unique_holder_names(holders).join(", ")
        ),
        KernelHibernation::DisabledForUnidentifiedCause => {
            "disabled by the kernel, with no boot flag, lockdown or secret memory found".to_owned()
        }
        KernelHibernation::DisabledWithoutInspection => {
            "disabled by the kernel; enable the NixOS module to see which app".to_owned()
        }
    }
}

fn unique_holder_names(holders: &[SecretMemoryHolder]) -> Vec<&str> {
    let mut names: Vec<&str> = holders.iter().map(|holder| holder.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names
}

pub(super) fn unsupported_by_logind_text() -> &'static str {
    "logind reports this isn't supported on this machine"
}
