use sottomano_widgets::clock::Clock;

use crate::{
    fixtures::{audio, compute, gamepad, media, memory_board, voice_dictation},
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

pub fn media_group_menu(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::MenuMembers(vec![
        media::group(clock),
        audio::group(clock),
        voice_dictation::group(clock),
        gamepad::group(clock),
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
