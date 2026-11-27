use std::{fs, path::Path, time::Instant};

use sottomano_toml_config::{Config, screen_edges::Position};
use sottomano_widget_scenes::scripted_session::{
    FrameWatcher, RenderedFrame, SCALE_FACTOR, ScriptedSession,
};
use sottomano_widgets::clock::Clock;

use crate::{
    RECORDING_EXTENSION, RECORDINGS_DIRECTORY,
    animation_recording::frame_content_crop::{
        CapturedFrame, ContentCrop, FrameContent, FrameSize, PanelBand, encode_animation,
    },
    demo_script::Demo,
    recording_still::{encode_still, still_path},
};

mod frame_content_crop;

const PANEL_SHADOW_REACH_PX: f32 = 24.0;

struct FrameRecorder {
    background: [u8; 4],
    canvas: FrameSize,
    panel_band: PanelBand,
    frames: Vec<CapturedFrame>,
    last_observed_at: Instant,
}

impl FrameWatcher for FrameRecorder {
    fn frame_advanced(&mut self, mut rendered_frame: RenderedFrame<'_>) {
        self.last_observed_at = rendered_frame.clock.instant;
        let (content, panel_band_column) = FrameContent::differing_from(
            self.background,
            rendered_frame.screenshot(),
            self.canvas,
            &self.panel_band,
        );
        if self.frames.last().is_none_or(|previous| {
            previous.content != content || previous.panel_band_column != panel_band_column
        }) {
            self.frames.push(CapturedFrame {
                content,
                panel_band_column,
                captured_at: rendered_frame.clock.instant,
            });
        }
    }
}

fn panel_band_rows(demo: &Demo, start: Clock, config: &Config, canvas: FrameSize) -> PanelBand {
    let band_height =
        ((config.desktop_panels.height + PANEL_SHADOW_REACH_PX) * SCALE_FACTOR).round() as u32;
    let canvas_height = u32::from(canvas.height);
    let rows = match (demo.scene.group)(start).position() {
        Position::Top => 0..band_height,
        Position::Bottom => {
            canvas_height
                .checked_sub(band_height)
                .expect("the panel band fits the canvas")..canvas_height
        }
        Position::Left => 0..0,
    };
    PanelBand { rows }
}

pub fn record(demo: &'static Demo, start: Clock, output_root: &Path) {
    let mut unwatched = ScriptedSession::start(demo.scene, start);
    let margin_pixels = (unwatched.config.theme.spacing.group_gap * SCALE_FACTOR).round() as u16;
    let canvas = FrameSize::from(unwatched.canvas);
    let middle_right = (usize::from(canvas.height) / 2 * usize::from(canvas.width)
        + usize::from(canvas.width)
        - 1)
        * 4;
    let background: [u8; 4] = unwatched.screenshot()[middle_right..middle_right + 4]
        .try_into()
        .expect("a pixel has four channels");
    let panel_band = panel_band_rows(demo, start, &unwatched.config, canvas);
    let mut session = unwatched.watched_by(FrameRecorder {
        background,
        canvas,
        panel_band: panel_band.clone(),
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
    let crop = ContentCrop::around_content(&frames, canvas, background, margin_pixels, panel_band);
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
