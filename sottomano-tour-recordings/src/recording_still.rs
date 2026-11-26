use std::{
    fs,
    path::{Path, PathBuf},
};

use webp_animation::{Encoder, EncoderOptions, EncodingConfig, EncodingType, LossyEncodingConfig};

use crate::{RECORDING_EXTENSION, RECORDINGS_DIRECTORY};

pub(crate) const STILLS_DIRECTORY: &str = "stills";
const STILL_QUALITY: f32 = 82.0;
const STILL_METHOD: usize = 6;

pub(crate) fn still_path(output_root: &Path, demo_name: &str) -> PathBuf {
    output_root
        .join(RECORDINGS_DIRECTORY)
        .join(STILLS_DIRECTORY)
        .join(format!("{demo_name}.{RECORDING_EXTENSION}"))
}

pub(crate) fn encode_still(cropped_rgba: &[u8], width: u32, height: u32, still: &Path) {
    let options = EncoderOptions {
        encoding_config: Some(EncodingConfig {
            encoding_type: EncodingType::Lossy(LossyEncodingConfig::default()),
            quality: STILL_QUALITY,
            method: STILL_METHOD,
        }),
        ..EncoderOptions::default()
    };
    let mut encoder = Encoder::new_with_options((width, height), options)
        .expect("cropped recordings are never empty");
    encoder
        .add_frame(cropped_rgba, 0)
        .expect("the still matches the recording size");
    let webp_bytes = encoder.finalize(1).expect("still is encodable");
    if let Some(stills_directory) = still.parent() {
        fs::create_dir_all(stills_directory).expect("stills directory is creatable");
    }
    fs::write(still, &*webp_bytes).expect("stills directory is writable");
}
