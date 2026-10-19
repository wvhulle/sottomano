pub mod animation_recording;
pub mod demo_script;
mod fixtures;
mod hovered_widget;
pub mod readme_embedding;
mod scripted_frames;
pub mod showcase_program;
mod showcased_group;

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDING_EXTENSION: &str = "webp";
