use sottomano_widgets::clock::Clock;

use crate::{
    fixtures::{
        audio, caldav, ci_runner_tasks, claude_usage, compute, dark_mode, docker_engines,
        idle_lock, incus_instances, location, login_session, media, memory_board,
        nspawn_containers, podman_engines, tailnet, virtual_machines, voice_dictation, weather,
        wireguard_tunnels,
    },
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

pub fn media_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![
        media::group(clock),
        audio::group(clock),
        voice_dictation::group(clock),
    ])
}

pub fn hardware_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![compute::group(clock), memory_board::group(clock)])
}

pub fn hardware_menu_runaway_memory_live(clock: Clock) -> Vec<ShowcaseMessage> {
    compute::live(clock)
        .into_iter()
        .chain(memory_board::runaway_growth_live(clock))
        .collect()
}

pub fn hardware_menu_live(clock: Clock) -> Vec<ShowcaseMessage> {
    compute::live(clock)
        .into_iter()
        .chain(memory_board::live(clock))
        .collect()
}

pub fn network_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![tailnet::group(clock), wireguard_tunnels::group(clock)])
}

pub fn virtualization_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![
        virtual_machines::group(clock),
        incus_instances::group(clock),
        nspawn_containers::group(clock),
        podman_engines::group(clock),
        docker_engines::group(clock),
    ])
}

pub fn agenda_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![
        caldav::group(clock),
        weather::group(clock),
        location::group(clock),
    ])
}

pub fn display_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![dark_mode::group(clock)])
}

pub fn services_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![
        ci_runner_tasks::group(clock),
        claude_usage::group(clock),
    ])
}

pub fn session_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![idle_lock::group(clock), login_session::group(clock)])
}
