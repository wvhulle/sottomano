pub mod animation_recording;
mod backdrop_photo;
pub mod demo_script;
mod fixtures;
mod frosted_backdrop;
mod hovered_widget;
mod scripted_frames;
pub mod showcase_message;
pub mod showcase_program;
mod showcased_group;

pub const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDING_EXTENSION: &str = "webp";
