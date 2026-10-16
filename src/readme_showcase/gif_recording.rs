use std::{
    borrow::Cow,
    fs::File,
    path::Path,
    time::{Duration, Instant},
};

use gif::{Encoder, Frame, Repeat};
use iced::{
    Point,
    futures::channel::mpsc::{self, Receiver},
    mouse::Button,
    window::Screenshot,
};
use iced_test::{
    Emulator, Instruction,
    emulator::{Event, Mode},
    instruction::{Interaction, Mouse, Target},
};

use crate::{
    config::Config,
    readme_showcase::{
        scene_script::{Scene, Step},
        showcase_program::ShowcaseProgram,
    },
};

const SCALE_FACTOR: f32 = 2.0;
const QUANTIZATION_SPEED: i32 = 10;
const EMULATOR_EVENT_CAPACITY: usize = 256;
const LAST_FRAME_DURATION: Duration = Duration::from_millis(100);
const FRAME_INTERVAL: Duration = Duration::from_millis(40);

struct CapturedFrame {
    screenshot: Screenshot,
    captured_at: Instant,
}

struct SceneRecorder {
    program: ShowcaseProgram,
    emulator: Emulator<ShowcaseProgram>,
    events: Receiver<Event<ShowcaseProgram>>,
    pointer: Point,
}

impl SceneRecorder {
    fn start(scene: &'static Scene, config: Config) -> Self {
        let program = ShowcaseProgram { scene, config };
        let (sender, events) = mpsc::channel(EMULATOR_EVENT_CAPACITY);
        let emulator = Emulator::new(sender, &program, Mode::Immediate, scene.canvas);
        SceneRecorder {
            program,
            emulator,
            events,
            pointer: Point::ORIGIN,
        }
    }

    fn settle(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            if let Event::Action(action) = event {
                self.emulator.perform(&self.program, action);
            }
        }
    }

    fn interact(&mut self, mouse: Mouse) {
        self.emulator.run(
            &self.program,
            Instruction::Interact(Interaction::Mouse(mouse)),
        );
        self.settle();
    }

    fn perform(&mut self, step: &Step) -> Vec<CapturedFrame> {
        match step {
            Step::PointAt { at, .. } => {
                self.pointer = *at;
                self.interact(Mouse::Move(Target::Point(*at)));
            }
            Step::Press { .. } => {
                self.interact(Mouse::Click {
                    button: Button::Left,
                    target: Some(Target::Point(self.pointer)),
                });
                self.interact(Mouse::Move(Target::Point(self.pointer)));
            }
        }
        let deadline = Instant::now() + step.hold();
        std::iter::from_fn(|| (Instant::now() < deadline).then(|| self.capture())).collect()
    }

    fn capture(&mut self) -> CapturedFrame {
        let frame_started = Instant::now();
        self.settle();
        let theme = self
            .emulator
            .theme(&self.program)
            .expect("the showcase program always sets a theme");
        let captured_at = Instant::now();
        let screenshot = self
            .emulator
            .screenshot(&self.program, &theme, SCALE_FACTOR);
        std::thread::sleep(FRAME_INTERVAL.saturating_sub(frame_started.elapsed()));
        CapturedFrame {
            screenshot,
            captured_at,
        }
    }
}

#[derive(Clone, Copy)]
struct ChangedRegion {
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
}

impl ChangedRegion {
    fn whole(width: u16, height: u16) -> Self {
        ChangedRegion {
            left: 0,
            top: 0,
            right: width - 1,
            bottom: height - 1,
        }
    }

    fn pixel(x: u16, y: u16) -> Self {
        ChangedRegion {
            left: x,
            top: y,
            right: x,
            bottom: y,
        }
    }

    fn including(self, x: u16, y: u16) -> Self {
        ChangedRegion {
            left: self.left.min(x),
            top: self.top.min(y),
            right: self.right.max(x),
            bottom: self.bottom.max(y),
        }
    }

    fn between(previous: &[u8], current: &[u8], width: u16) -> Option<Self> {
        previous
            .chunks_exact(4)
            .zip(current.chunks_exact(4))
            .enumerate()
            .filter(|(_, (before, after))| before != after)
            .map(|(index, _)| {
                let x = u16::try_from(index % usize::from(width)).expect("column fits a GIF");
                let y = u16::try_from(index / usize::from(width)).expect("row fits a GIF");
                ChangedRegion::pixel(x, y)
            })
            .reduce(|region, changed| region.including(changed.left, changed.top))
    }

    fn width(self) -> u16 {
        self.right - self.left + 1
    }

    fn height(self) -> u16 {
        self.bottom - self.top + 1
    }

    fn crop(self, rgba: &[u8], canvas_width: u16) -> Vec<u8> {
        (self.top..=self.bottom)
            .flat_map(|row| {
                let start =
                    (usize::from(row) * usize::from(canvas_width) + usize::from(self.left)) * 4;
                rgba[start..start + usize::from(self.width()) * 4]
                    .iter()
                    .copied()
            })
            .collect()
    }
}

fn register_font(config: &Config) {
    iced_test::renderer::graphics::text::font_system()
        .write()
        .expect("font system lock is never poisoned")
        .load_font(Cow::Owned(config.theme.font.data.to_vec()));
}

fn encode_gif(frames: &[CapturedFrame], gif: &Path) {
    let Some(first) = frames.first() else {
        return;
    };
    let width = u16::try_from(first.screenshot.size.width).expect("frame width fits a GIF");
    let height = u16::try_from(first.screenshot.size.height).expect("frame height fits a GIF");
    let file = File::create(gif).expect("docs directory is writable");
    let mut encoder = Encoder::new(file, width, height, &[]).expect("GIF header is writable");
    encoder
        .set_repeat(Repeat::Infinite)
        .expect("GIF repeat extension is writable");
    let distinct: Vec<&CapturedFrame> = frames
        .chunk_by(|earlier, later| earlier.screenshot.rgba == later.screenshot.rgba)
        .map(|repeats| &repeats[0])
        .collect();
    let shown_until: Vec<Instant> = distinct
        .iter()
        .skip(1)
        .map(|frame| frame.captured_at)
        .chain([frames[frames.len() - 1].captured_at + LAST_FRAME_DURATION])
        .collect();
    let previous_frames = std::iter::once(None).chain(distinct.iter().copied().map(Some));
    distinct
        .iter()
        .zip(previous_frames)
        .zip(shown_until)
        .for_each(|((captured, previous), shown_until)| {
            let region = previous
                .and_then(|previous| {
                    ChangedRegion::between(
                        &previous.screenshot.rgba,
                        &captured.screenshot.rgba,
                        width,
                    )
                })
                .unwrap_or_else(|| ChangedRegion::whole(width, height));
            let mut rgba = region.crop(&captured.screenshot.rgba, width);
            let mut frame = Frame::from_rgba_speed(
                region.width(),
                region.height(),
                &mut rgba,
                QUANTIZATION_SPEED,
            );
            frame.left = region.left;
            frame.top = region.top;
            let centiseconds = (shown_until - captured.captured_at).as_millis() / 10;
            frame.delay = u16::try_from(centiseconds.max(1)).unwrap_or(u16::MAX);
            encoder.write_frame(&frame).expect("GIF frame is writable");
        });
}

pub fn record(scene: &'static Scene) {
    let config = Config::fallback();
    register_font(&config);
    let mut recorder = SceneRecorder::start(scene, config);
    recorder.settle();
    let frames: Vec<CapturedFrame> = scene
        .steps
        .iter()
        .flat_map(|step| recorder.perform(step))
        .collect();
    let gif = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join(format!("{}.gif", scene.name));
    encode_gif(&frames, &gif);
}
