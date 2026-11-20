use sottomano_widgets::clock::Clock;

use crate::{
    fixtures::{
        audio, compute, docker_engines, incus_instances, media, memory_board,
        nspawn_containers, podman_engines, tailnet, virtual_machines, voice_dictation,
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
