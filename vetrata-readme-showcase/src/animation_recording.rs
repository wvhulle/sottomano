use std::{
    borrow::Cow,
    fs,
    iter::from_fn,
    path::Path,
    thread::sleep,
    time::{Duration, Instant},
};

use iced::{
    Point, Size, Vector,
    advanced::widget::operation::{Operation, Outcome, black_box},
    futures::{
        channel::mpsc::{self, Receiver},
        executor::block_on,
    },
    mouse::Button,
};
use iced_test::{
    Emulator, Instruction, Selector,
    core::renderer::Headless,
    emulator::{Event, Mode},
    instruction::{Interaction, Mouse, Target},
    program::Program,
    renderer::graphics::text::font_system,
    runtime::{UserInterface, user_interface::Cache},
};
use vetrata_config::Config;
use vetrata_widgets::hover_tooltips::HoverTarget;
use webp_animation::{Encoder, EncoderOptions, EncodingConfig, EncodingType};

use crate::{
    RECORDING_EXTENSION, RECORDINGS_DIRECTORY, WORKSPACE_ROOT,
    demo_script::{Demo, Step},
    hovered_widget::{AnyHoveredWidget, HoveredWidget},
    readme_embedding::RecordedAnimation,
    showcase_program::ShowcaseProgram,
};

const CANVAS: Size = Size::new(720.0, 960.0);
const SCALE_FACTOR: f32 = 2.0;
const REST_POINTER: Point = Point::new(CANVAS.width - 1.0, CANVAS.height / 2.0);
const REST_HOLD: Duration = Duration::from_millis(1000);
const HOVER_HOLD: Duration = Duration::from_millis(2500);
const PRESS_LEAD: Duration = Duration::from_millis(1000);
const PRESS_HOLD: Duration = Duration::from_millis(1500);
const LIVE_INTERVAL: Duration = Duration::from_millis(1500);
const DRAG_MOVES: u16 = 16;
const DRAG_MOVE_HOLD: Duration = Duration::from_millis(60);
const FRAME_INTERVAL: Duration = Duration::from_millis(20);
const LAST_FRAME_DURATION: Duration = Duration::from_millis(100);
const LOSSLESS_EFFORT: f32 = 75.0;
const LOSSLESS_METHOD: usize = 4;
const EMULATOR_EVENT_CAPACITY: usize = 256;

struct CapturedFrame {
    rgba: Vec<u8>,
    captured_at: Instant,
}

struct DemoRecorder {
    program: ShowcaseProgram,
    emulator: Emulator<ShowcaseProgram>,
    events: Receiver<Event<ShowcaseProgram>>,
    layout_renderer: iced::Renderer,
    pointer: Point,
    live_tick: u32,
    next_live_delivery: Instant,
}

impl DemoRecorder {
    fn start(demo: &'static Demo, config: Config) -> Self {
        let program = ShowcaseProgram { demo, config };
        let (sender, events) = mpsc::channel(EMULATOR_EVENT_CAPACITY);
        let emulator = Emulator::new(sender, &program, Mode::Immediate, CANVAS);
        let settings = program.settings();
        let layout_renderer = block_on(iced::Renderer::new(
            settings.default_font,
            settings.default_text_size,
            None,
        ))
        .expect("a headless renderer is available");
        let mut recorder = DemoRecorder {
            program,
            emulator,
            events,
            layout_renderer,
            pointer: REST_POINTER,
            live_tick: 1,
            next_live_delivery: Instant::now() + LIVE_INTERVAL,
        };
        recorder.settle();
        recorder
    }

    fn deliver_live_readings(&mut self) {
        if Instant::now() < self.next_live_delivery {
            return;
        }
        (self.program.demo.live)(self.live_tick)
            .into_iter()
            .for_each(|reading| self.emulator.update(&self.program, reading));
        self.live_tick += 1;
        self.next_live_delivery += LIVE_INTERVAL;
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

    fn point_at(&mut self, pointer: Point) {
        self.pointer = pointer;
        self.interact(Mouse::Move(Target::Point(pointer)));
    }

    fn click(&mut self) {
        self.interact(Mouse::Click {
            button: Button::Left,
            target: Some(Target::Point(self.pointer)),
        });
        self.interact(Mouse::Move(Target::Point(self.pointer)));
    }

    fn locate(&mut self, target: &HoverTarget) -> Point {
        let mut interface = UserInterface::build(
            self.emulator.view(&self.program),
            CANVAS,
            Cache::default(),
            &mut self.layout_renderer,
        );
        let mut finder = HoveredWidget(target.clone()).find();
        interface.operate(&self.layout_renderer, &mut black_box(&mut finder));
        if let Outcome::Some(Some(bounds)) = finder.finish() {
            return bounds.center();
        }
        let mut shown = AnyHoveredWidget.find_all();
        interface.operate(&self.layout_renderer, &mut black_box(&mut shown));
        let shown = match shown.finish() {
            Outcome::Some(targets) => targets,
            _ => Vec::new(),
        };
        panic!(
            "demo {} hovers {target:?}, which no widget shows; shown now: {shown:?}",
            self.program.demo.name
        )
    }

    fn rest(&mut self) -> Vec<CapturedFrame> {
        self.point_at(REST_POINTER);
        self.capture_during(REST_HOLD)
    }

    fn perform(&mut self, step: &Step) -> Vec<CapturedFrame> {
        match step {
            Step::Hover(target) => {
                let pointer = self.locate(target);
                self.point_at(pointer);
                self.capture_during(HOVER_HOLD)
            }
            Step::Press(target) => {
                let pointer = self.locate(target);
                self.point_at(pointer);
                let lead = self.capture_during(PRESS_LEAD);
                self.click();
                lead.into_iter()
                    .chain(self.capture_during(PRESS_HOLD))
                    .collect()
            }
            Step::Drag { target, by } => {
                let grabbed = self.locate(target);
                self.point_at(grabbed);
                let lead = self.capture_during(PRESS_LEAD);
                self.interact(Mouse::Press {
                    button: Button::Left,
                    target: Some(Target::Point(grabbed)),
                });
                let dragged: Vec<CapturedFrame> = (1..=DRAG_MOVES)
                    .flat_map(|moved| {
                        let progress = f32::from(moved) / f32::from(DRAG_MOVES);
                        self.point_at(grabbed + Vector::new(by * progress, 0.0));
                        self.capture_during(DRAG_MOVE_HOLD)
                    })
                    .collect();
                self.interact(Mouse::Release {
                    button: Button::Left,
                    target: Some(Target::Point(self.pointer)),
                });
                lead.into_iter()
                    .chain(dragged)
                    .chain(self.capture_during(HOVER_HOLD))
                    .collect()
            }
        }
    }

    fn capture_during(&mut self, hold: Duration) -> Vec<CapturedFrame> {
        let deadline = Instant::now() + hold;
        from_fn(|| (Instant::now() < deadline).then(|| self.capture())).collect()
    }

    fn capture(&mut self) -> CapturedFrame {
        let frame_started = Instant::now();
        self.deliver_live_readings();
        self.settle();
        let theme = self
            .emulator
            .theme(&self.program)
            .expect("the showcase program always sets a theme");
        let captured_at = Instant::now();
        let screenshot = self
            .emulator
            .screenshot(&self.program, &theme, SCALE_FACTOR);
        sleep(FRAME_INTERVAL.saturating_sub(frame_started.elapsed()));
        CapturedFrame {
            rgba: screenshot.rgba.to_vec(),
            captured_at,
        }
    }
}

#[derive(Clone, Copy)]
struct PixelRegion {
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
}

impl PixelRegion {
    fn whole(width: u16, height: u16) -> Self {
        PixelRegion {
            left: 0,
            top: 0,
            right: width - 1,
            bottom: height - 1,
        }
    }

    fn pixel(x: u16, y: u16) -> Self {
        PixelRegion {
            left: x,
            top: y,
            right: x,
            bottom: y,
        }
    }

    fn union(self, other: Self) -> Self {
        PixelRegion {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn widened(self, margin: u16, within: Self) -> Self {
        PixelRegion {
            left: self.left.saturating_sub(margin).max(within.left),
            top: self.top.saturating_sub(margin).max(within.top),
            right: self.right.saturating_add(margin).min(within.right),
            bottom: self.bottom.saturating_add(margin).min(within.bottom),
        }
    }

    fn differing(reference: &[u8], current: &[u8], width: u16) -> Option<Self> {
        reference
            .as_chunks::<4>()
            .0
            .iter()
            .zip(current.as_chunks::<4>().0)
            .enumerate()
            .filter(|(_, (before, after))| before != after)
            .map(|(index, _)| {
                let x = u16::try_from(index % usize::from(width)).expect("column fits a GIF");
                let y = u16::try_from(index / usize::from(width)).expect("row fits a GIF");
                PixelRegion::pixel(x, y)
            })
            .reduce(PixelRegion::union)
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

#[derive(Clone, Copy)]
struct FrameSize {
    width: u16,
    height: u16,
}

impl FrameSize {
    fn of_canvas() -> Self {
        let physical = |logical: f32| {
            let pixels = (logical * SCALE_FACTOR).round();
            u16::try_from(pixels as u32).expect("canvas fits a GIF")
        };
        FrameSize {
            width: physical(CANVAS.width),
            height: physical(CANVAS.height),
        }
    }
}

struct CroppedRecording {
    frames: Vec<CapturedFrame>,
    size: FrameSize,
}

fn cropped_to_content(
    frames: Vec<CapturedFrame>,
    canvas: FrameSize,
    margin: u16,
) -> CroppedRecording {
    let whole = PixelRegion::whole(canvas.width, canvas.height);
    let background: Vec<u8> = frames
        .first()
        .map(|frame| {
            let top_right = (usize::from(canvas.width) - 1) * 4;
            frame.rgba[top_right..top_right + 4].repeat(frame.rgba.len() / 4)
        })
        .unwrap_or_default();
    let content = frames
        .iter()
        .filter_map(|frame| PixelRegion::differing(&background, &frame.rgba, canvas.width))
        .reduce(PixelRegion::union)
        .map_or(whole, |content| content.widened(margin, whole));
    CroppedRecording {
        frames: frames
            .into_iter()
            .map(|frame| CapturedFrame {
                rgba: content.crop(&frame.rgba, canvas.width),
                captured_at: frame.captured_at,
            })
            .collect(),
        size: FrameSize {
            width: content.width(),
            height: content.height(),
        },
    }
}

fn register_font(config: &Config) {
    font_system()
        .write()
        .expect("font system lock is never poisoned")
        .load_font(Cow::Owned(config.theme.font.data.to_vec()));
}

fn encode_animation(frames: &[CapturedFrame], size: FrameSize, animation: &Path) {
    let Some(first) = frames.first() else {
        return;
    };
    let Some(last) = frames.last() else {
        return;
    };
    let milliseconds_since_first = |at: Instant| {
        i32::try_from((at - first.captured_at).as_millis()).expect("recordings are short")
    };
    let options = EncoderOptions {
        encoding_config: Some(EncodingConfig {
            encoding_type: EncodingType::Lossless,
            quality: LOSSLESS_EFFORT,
            method: LOSSLESS_METHOD,
        }),
        ..EncoderOptions::default()
    };
    let encoder =
        Encoder::new_with_options((u32::from(size.width), u32::from(size.height)), options)
            .expect("cropped recordings are never empty");
    let webp_bytes = frames
        .chunk_by(|earlier, later| earlier.rgba == later.rgba)
        .map(|repeats| &repeats[0])
        .fold(encoder, |mut encoder, frame| {
            encoder
                .add_frame(&frame.rgba, milliseconds_since_first(frame.captured_at))
                .expect("frames match the recording size");
            encoder
        })
        .finalize(milliseconds_since_first(
            last.captured_at + LAST_FRAME_DURATION,
        ))
        .expect("animation is encodable");
    fs::write(animation, &*webp_bytes).expect("recordings directory is writable");
}

pub fn record(demo: &'static Demo) -> RecordedAnimation {
    let config = Config::fallback();
    register_font(&config);
    let margin_pixels = (config.layout.overlay.panel_gap * SCALE_FACTOR).round() as u16;
    let mut recorder = DemoRecorder::start(demo, config);
    let opening = recorder.rest();
    let scripted: Vec<CapturedFrame> = demo
        .script
        .iter()
        .flat_map(|step| recorder.perform(step))
        .collect();
    let closing = recorder.rest();
    let frames: Vec<CapturedFrame> = opening.into_iter().chain(scripted).chain(closing).collect();
    let cropped = cropped_to_content(frames, FrameSize::of_canvas(), margin_pixels);
    let animation = Path::new(WORKSPACE_ROOT)
        .join(RECORDINGS_DIRECTORY)
        .join(format!("{}.{RECORDING_EXTENSION}", demo.name));
    encode_animation(&cropped.frames, cropped.size, &animation);
    RecordedAnimation {
        name: demo.name,
        logical_width: (f32::from(cropped.size.width) / SCALE_FACTOR).round() as u32,
    }
}
