pub mod animation_recording;
pub mod demo_script;
mod fixtures;
mod hovered_widget;
mod scripted_frames;
pub mod showcase_program;
mod showcased_group;
pub mod tour_document_embedding;

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDING_EXTENSION: &str = "webp";
