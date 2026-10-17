pub mod audio;
pub mod claude_usage;
pub mod compute;
pub mod idle_lock;
pub mod media;
pub mod niri_session;
pub mod wallpaper;
pub mod weather;

use crate::desktop_shell::Message;

pub fn cycled<T: Copy>(values: &[T], tick: u32) -> T {
    values[usize::try_from(tick).unwrap_or_default() % values.len()]
}

pub fn unchanging(_tick: u32) -> Vec<Message> {
    Vec::new()
}
