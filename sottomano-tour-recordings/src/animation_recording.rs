use std::{fs, path::Path, time::Instant};

use sottomano_widget_scenes::scripted_session::{
    FrameWatcher, RenderedFrame, SCALE_FACTOR, ScriptedSession,
};
use sottomano_widgets::clock::Clock;

use crate::{
    RECORDING_EXTENSION, RECORDINGS_DIRECTORY,
    animation_recording::frame_content_crop::{
        CapturedFrame, ContentCrop, FrameContent, FrameSize, encode_animation,
    },
    demo_script::Demo,
    recording_still::{encode_still, still_path},
};

mod frame_content_crop;

struct FrameRecorder {
    background: [u8; 4],
    canvas: FrameSize,
    frames: Vec<CapturedFrame>,
    last_observed_at: Instant,
}

impl FrameWatcher for FrameRecorder {
    fn frame_advanced(&mut self, mut rendered_frame: RenderedFrame<'_>) {
        self.last_observed_at = rendered_frame.clock.instant;
        let content =
            FrameContent::differing_from(self.background, rendered_frame.screenshot(), self.canvas);
        if self
            .frames
            .last()
            .is_none_or(|previous| previous.content != content)
        {
            self.frames.push(CapturedFrame {
                content,
                captured_at: rendered_frame.clock.instant,
            });
        }
    }
}

pub fn record(demo: &'static Demo, start: Clock, output_root: &Path) {
    let mut unwatched = ScriptedSession::start(demo.scene, start);
    let margin_pixels = (unwatched.config.theme.spacing.group_gap * SCALE_FACTOR).round() as u16;
    let canvas = FrameSize::from(unwatched.canvas);
    let top_right = (usize::from(canvas.width) - 1) * 4;
    let background: [u8; 4] = unwatched.screenshot()[top_right..top_right + 4]
        .try_into()
        .expect("a pixel has four channels");
    let mut session = unwatched.watched_by(FrameRecorder {
        background,
        canvas,
        frames: Vec::new(),
        last_observed_at: start.instant,
    });
    session.rest();
    demo.script.iter().for_each(|step| session.perform(step));
    session.hold_end_state();
    let end_state_position = session
        .watcher
        .frames
        .len()
        .checked_sub(1)
        .expect("the opening rest captures at least one frame");
    session.press_away();
    let FrameRecorder {
        frames,
        last_observed_at,
        ..
    } = session.watcher;
    let crop = ContentCrop::around_content(&frames, canvas, background, margin_pixels);
    let recordings_directory = output_root.join(RECORDINGS_DIRECTORY);
    fs::create_dir_all(&recordings_directory).expect("recordings directory is creatable");
    let animation = recordings_directory.join(format!("{}.{RECORDING_EXTENSION}", demo.name));
    encode_animation(&frames, last_observed_at, &crop, &animation);
    encode_still(
        &crop.crop(&frames[end_state_position]),
        u32::from(crop.size.width),
        u32::from(crop.size.height),
        &still_path(output_root, demo.name),
    );
}
