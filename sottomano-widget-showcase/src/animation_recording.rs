use std::{borrow::Cow, fs, path::Path, time::Duration};

use iced::{Size, futures::executor::block_on};
use iced_test::{
    core::renderer::Headless, program::Program, renderer::graphics::text::font_system,
};
use sottomano_toml_config::Config;
use sottomano_widgets::clock::Clock;

use crate::{
    RECORDING_EXTENSION, RECORDINGS_DIRECTORY,
    animation_recording::{
        demo_recorder::{DemoRecorder, FrameCapture},
        frame_content_crop::{CapturedFrame, ContentCrop, FrameSize, encode_animation},
    },
    demo_script::Demo,
    recording_still::{encode_still, still_path},
    showcase_program::ShowcaseProgram,
};

mod demo_recorder;
mod frame_content_crop;

pub(crate) const CANVAS: Size = Size::new(1280.0, 1080.0);
pub(crate) const PHOTO_CANVAS: Size = Size::new(1000.0, 320.0);
const SCALE_FACTOR: f32 = 2.0;
pub(crate) const FRAME_INTERVAL: Duration = Duration::from_millis(20);

fn register_font(config: &Config) {
    font_system()
        .write()
        .expect("font system lock is never poisoned")
        .load_font(Cow::Owned(config.theme.font.data.to_vec()));
}

pub struct RecordingSession {
    config: Config,
    layout_renderer: iced::Renderer,
}

impl RecordingSession {
    #[must_use]
    pub fn open(start: Clock) -> Self {
        let config = Config::default();
        register_font(&config);
        let settings = ShowcaseProgram::scripted(&config, start).settings();
        let layout_renderer = block_on(iced::Renderer::new(
            settings.default_font,
            settings.default_text_size,
            None,
        ))
        .expect("a headless renderer is available");
        RecordingSession {
            config,
            layout_renderer,
        }
    }

    pub fn record(&mut self, demo: &'static Demo, start: Clock, output_root: &Path) {
        let margin_pixels = (self.config.theme.spacing.group_gap * SCALE_FACTOR).round() as u16;
        let mut recorder = DemoRecorder::start(
            demo,
            &self.config,
            start,
            &mut self.layout_renderer,
            FrameCapture::Screenshots,
        );
        let opening = recorder.rest();
        let scripted: Vec<CapturedFrame> = demo
            .script
            .iter()
            .flat_map(|step| recorder.perform(step))
            .collect();
        let closing = recorder.press_away();
        let final_scripted_position = (opening.len() + scripted.len())
            .checked_sub(1)
            .expect("the opening rest captures at least one frame");
        let frames: Vec<CapturedFrame> =
            opening.into_iter().chain(scripted).chain(closing).collect();
        let crop = ContentCrop::around_content(
            &frames,
            FrameSize::from(demo.backdrop.canvas()),
            recorder.background,
            margin_pixels,
        );
        drop(recorder);
        let recordings_directory = output_root.join(RECORDINGS_DIRECTORY);
        fs::create_dir_all(&recordings_directory).expect("recordings directory is creatable");
        let animation = recordings_directory.join(format!("{}.{RECORDING_EXTENSION}", demo.name));
        encode_animation(&frames, &crop, &animation);
        encode_still(
            &crop.crop(&frames[final_scripted_position]),
            u32::from(crop.size.width),
            u32::from(crop.size.height),
            &still_path(output_root, demo.name),
        );
    }

    pub fn rehearse(&mut self, demo: &'static Demo, start: Clock) {
        let mut recorder = DemoRecorder::start(
            demo,
            &self.config,
            start,
            &mut self.layout_renderer,
            FrameCapture::Rehearsal,
        );
        let _opening = recorder.rest();
        demo.script.iter().for_each(|step| {
            let _rehearsed = recorder.perform(step);
        });
        let _closing = recorder.press_away();
    }
}
