use std::{
    borrow::Cow,
    fs,
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

use iced::{
    Point, Rectangle, Size, Vector,
    advanced::widget::operation::{Operation, Outcome, black_box},
    futures::{
        StreamExt,
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
use sottomano_config::Config;
use sottomano_widgets::clock::Clock;
use webp_animation::{Encoder, EncoderOptions, EncodingConfig, EncodingType};

use crate::{
    RECORDING_EXTENSION, RECORDINGS_DIRECTORY, WORKSPACE_ROOT,
    demo_script::{Demo, Step},
    hovered_widget::{AnyHoveredWidget, HoveredWidget},
    showcase_message::ShowcaseMessage,
    showcase_program::ShowcaseProgram,
    tour_document_embedding::RecordedAnimation,
};

pub(crate) const CANVAS: Size = Size::new(720.0, 960.0);
const SCALE_FACTOR: f32 = 2.0;
const REST_POINTER: Point = Point::new(CANVAS.width - 1.0, CANVAS.height / 2.0);
const REST_HOLD: Duration = Duration::from_millis(1000);
const HOVER_HOLD: Duration = Duration::from_millis(2500);
const PRESS_LEAD: Duration = Duration::from_millis(1000);
const PRESS_HOLD: Duration = Duration::from_millis(1500);
const DRAG_MOVES: u16 = 16;
const DRAG_MOVE_HOLD: Duration = Duration::from_millis(60);
pub(crate) const FRAME_INTERVAL: Duration = Duration::from_millis(20);
const LAST_FRAME_DURATION: Duration = Duration::from_millis(100);
const LOSSLESS_EFFORT: f32 = 75.0;
const LOSSLESS_METHOD: usize = 4;
const EMULATOR_EVENT_CAPACITY: usize = 256;

struct CapturedFrame {
    content: Option<ContentRows>,
    captured_at: Instant,
}

struct DemoRecorder {
    demo: &'static Demo,
    program: ShowcaseProgram,
    emulator: Emulator<ShowcaseProgram>,
    events: Receiver<Event<ShowcaseProgram>>,
    layout_renderer: iced::Renderer,
    pointer: Point,
    background: [u8; 4],
    clock: Clock,
}

impl DemoRecorder {
    fn start(demo: &'static Demo, config: &Config, clock: Clock) -> Self {
        let program = ShowcaseProgram::scripted(config, clock);
        let (sender, events) = mpsc::channel(EMULATOR_EVENT_CAPACITY);
        let mut emulator = Emulator::with_preset(
            sender,
            &program,
            Mode::Zen,
            CANVAS,
            Some(program.preset(demo)),
        );
        let settings = program.settings();
        let layout_renderer = block_on(iced::Renderer::new(
            settings.default_font,
            settings.default_text_size,
            None,
        ))
        .expect("a headless renderer is available");
        let theme = emulator
            .theme(&program)
            .expect("the showcase program always sets a theme");
        let top_right = (usize::from(FrameSize::of_canvas().width) - 1) * 4;
        let background = emulator.screenshot(&program, &theme, SCALE_FACTOR).rgba
            [top_right..top_right + 4]
            .try_into()
            .expect("a pixel has four channels");
        let mut recorder = DemoRecorder {
            demo,
            program,
            emulator,
            events,
            layout_renderer,
            pointer: REST_POINTER,
            background,
            clock,
        };
        recorder.settle();
        recorder
    }

    fn settle(&mut self) {
        loop {
            match block_on(self.events.next()) {
                Some(Event::Action(action)) => self.emulator.perform(&self.program, action),
                Some(Event::Failed(instruction)) => panic!(
                    "demo {} cannot {instruction}: its target is not on screen",
                    self.demo.name
                ),
                Some(Event::Ready) => return,
                None => panic!("demo {} lost its emulator", self.demo.name),
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

    fn locate(&mut self, selector: impl Selector<Output = Rectangle> + Send + 'static) -> Point {
        let mut interface = UserInterface::build(
            self.emulator.view(&self.program),
            CANVAS,
            Cache::default(),
            &mut self.layout_renderer,
        );
        let wanted = selector.description();
        let mut finder = selector.find_all();
        interface.operate(&self.layout_renderer, &mut black_box(&mut finder));
        if let Outcome::Some(matches) = finder.finish()
            && let Some(last_laid_out) = matches.last()
        {
            return last_laid_out.center();
        }
        let mut shown = AnyHoveredWidget.find_all();
        interface.operate(&self.layout_renderer, &mut black_box(&mut shown));
        let shown = match shown.finish() {
            Outcome::Some(targets) => targets,
            _ => Vec::new(),
        };
        panic!(
            "demo {} points at {wanted}, which is not on screen; hover areas shown now: {shown:?}",
            self.demo.name
        )
    }

    fn press_at(&mut self, pointer: Point) -> Vec<CapturedFrame> {
        self.point_at(pointer);
        let lead = self.capture_during(PRESS_LEAD);
        self.click();
        lead.into_iter()
            .chain(self.capture_during(PRESS_HOLD))
            .collect()
    }

    fn rest(&mut self) -> Vec<CapturedFrame> {
        self.point_at(REST_POINTER);
        self.capture_during(REST_HOLD)
    }

    fn perform(&mut self, step: &Step) -> Vec<CapturedFrame> {
        match step {
            Step::Hover(target) => {
                let pointer = self.locate(HoveredWidget(target.clone()));
                self.point_at(pointer);
                self.capture_during(HOVER_HOLD)
            }
            Step::Press(target) => {
                let pointer = self.locate(HoveredWidget(target.clone()));
                self.press_at(pointer)
            }
            Step::HoverText(shown) => {
                self.interact(Mouse::Move(Target::Text((*shown).to_owned())));
                self.capture_during(HOVER_HOLD)
            }
            Step::PressText(shown) => {
                self.interact(Mouse::Move(Target::Text((*shown).to_owned())));
                let lead = self.capture_during(PRESS_LEAD);
                self.interact(Mouse::Click {
                    button: Button::Left,
                    target: Some(Target::Text((*shown).to_owned())),
                });
                lead.into_iter()
                    .chain(self.capture_during(PRESS_HOLD))
                    .collect()
            }
            Step::RestFor(hold) => {
                self.point_at(REST_POINTER);
                self.capture_during(*hold)
            }
            Step::Drag { target, by } => {
                let grabbed = self.locate(HoveredWidget(target.clone()));
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
        let frame_count = hold.as_millis() / FRAME_INTERVAL.as_millis();
        (0..frame_count).map(|_| self.capture()).fold(
            Vec::new(),
            |mut frames: Vec<CapturedFrame>, frame| {
                if frames
                    .last()
                    .is_none_or(|previous| previous.content != frame.content)
                {
                    frames.push(frame);
                }
                frames
            },
        )
    }

    fn screenshot(&mut self) -> Vec<u8> {
        let theme = self
            .emulator
            .theme(&self.program)
            .expect("the showcase program always sets a theme");
        self.emulator
            .screenshot(&self.program, &theme, SCALE_FACTOR)
            .rgba
            .to_vec()
    }

    fn capture(&mut self) -> CapturedFrame {
        self.clock = self.clock.advanced(FRAME_INTERVAL);
        self.emulator
            .update(&self.program, ShowcaseMessage::Frame(self.clock));
        let rgba = self.screenshot();
        CapturedFrame {
            content: ContentRows::differing_from(self.background, &rgba, FrameSize::of_canvas()),
            captured_at: self.clock.instant,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct PixelSpan {
    first: u16,
    last: u16,
}

impl PixelSpan {
    fn whole(length: u16) -> Self {
        PixelSpan {
            first: 0,
            last: length - 1,
        }
    }

    fn single(position: u16) -> Self {
        PixelSpan {
            first: position,
            last: position,
        }
    }

    fn union(self, other: Self) -> Self {
        PixelSpan {
            first: self.first.min(other.first),
            last: self.last.max(other.last),
        }
    }

    fn widened(self, margin: u16, within: Self) -> Self {
        PixelSpan {
            first: self.first.saturating_sub(margin).max(within.first),
            last: self.last.saturating_add(margin).min(within.last),
        }
    }

    fn aligned_to_logical_pixels(self, within: Self) -> Self {
        let pixels_per_logical = SCALE_FACTOR as u16;
        let first = self.first - self.first % pixels_per_logical;
        let past_last = (self.last + 1).next_multiple_of(pixels_per_logical);
        PixelSpan {
            first: first.max(within.first),
            last: (past_last - 1).min(within.last),
        }
    }

    fn length(self) -> u16 {
        self.last - self.first + 1
    }

    fn contains(self, position: u16) -> bool {
        (self.first..=self.last).contains(&position)
    }

    fn byte_range(self) -> Range<usize> {
        usize::from(self.first) * 4..(usize::from(self.last) + 1) * 4
    }
}

#[derive(PartialEq)]
struct ContentRows {
    rows: PixelSpan,
    columns: PixelSpan,
    rgba: Vec<u8>,
}

impl ContentRows {
    fn differing_from(background: [u8; 4], rgba: &[u8], canvas: FrameSize) -> Option<Self> {
        let row_bytes = usize::from(canvas.width) * 4;
        let (rows, columns) = rgba
            .chunks_exact(row_bytes)
            .enumerate()
            .filter_map(|(y, row)| {
                let pixels = row.as_chunks::<4>().0;
                let first_x = pixels.iter().position(|pixel| *pixel != background)?;
                let last_x = pixels.iter().rposition(|pixel| *pixel != background)?;
                let pixel_position =
                    |position: usize| u16::try_from(position).expect("pixel fits a recording");
                Some((
                    PixelSpan::single(pixel_position(y)),
                    PixelSpan {
                        first: pixel_position(first_x),
                        last: pixel_position(last_x),
                    },
                ))
            })
            .reduce(|(rows, columns), (row, row_columns)| {
                (rows.union(row), columns.union(row_columns))
            })?;
        let start = usize::from(rows.first) * row_bytes;
        let end = (usize::from(rows.last) + 1) * row_bytes;
        Some(ContentRows {
            rows,
            columns,
            rgba: rgba[start..end].to_vec(),
        })
    }

    fn row(&self, y: u16, row_bytes: usize) -> Option<&[u8]> {
        self.rows.contains(y).then(|| {
            let start = usize::from(y - self.rows.first) * row_bytes;
            &self.rgba[start..start + row_bytes]
        })
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

struct ContentCrop {
    shown_rows: PixelSpan,
    shown_columns: PixelSpan,
    row_bytes: usize,
    background_row: Vec<u8>,
    size: FrameSize,
}

impl ContentCrop {
    fn around_content(
        frames: &[CapturedFrame],
        canvas: FrameSize,
        background: [u8; 4],
        margin: u16,
    ) -> Self {
        let all_rows = PixelSpan::whole(canvas.height);
        let all_columns = PixelSpan::whole(canvas.width);
        let contents = || frames.iter().filter_map(|frame| frame.content.as_ref());
        let shown_rows = contents()
            .map(|content| content.rows)
            .reduce(PixelSpan::union)
            .map_or(all_rows, |span| span.widened(margin, all_rows));
        let shown_columns = contents()
            .map(|content| content.columns)
            .reduce(PixelSpan::union)
            .map_or(all_columns, |span| {
                span.widened(margin, all_columns)
                    .aligned_to_logical_pixels(all_columns)
            });
        ContentCrop {
            shown_rows,
            shown_columns,
            row_bytes: usize::from(canvas.width) * 4,
            background_row: background.repeat(usize::from(canvas.width)),
            size: FrameSize {
                width: shown_columns.length(),
                height: shown_rows.length(),
            },
        }
    }

    fn crop(&self, frame: &CapturedFrame) -> Vec<u8> {
        (self.shown_rows.first..=self.shown_rows.last)
            .flat_map(|y| {
                frame
                    .content
                    .as_ref()
                    .and_then(|content| content.row(y, self.row_bytes))
                    .unwrap_or(&self.background_row)[self.shown_columns.byte_range()]
                .iter()
                .copied()
            })
            .collect()
    }

    fn logical_width(&self) -> u16 {
        self.size.width / SCALE_FACTOR as u16
    }
}

fn register_font(config: &Config) {
    font_system()
        .write()
        .expect("font system lock is never poisoned")
        .load_font(Cow::Owned(config.theme.font.data.to_vec()));
}

fn encode_animation(frames: &[CapturedFrame], crop: &ContentCrop, animation: &Path) {
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
    let encoder = Encoder::new_with_options(
        (u32::from(crop.size.width), u32::from(crop.size.height)),
        options,
    )
    .expect("cropped recordings are never empty");
    let webp_bytes = frames
        .chunk_by(|earlier, later| earlier.content == later.content)
        .map(|repeats| &repeats[0])
        .fold(encoder, |mut encoder, frame| {
            encoder
                .add_frame(
                    &crop.crop(frame),
                    milliseconds_since_first(frame.captured_at),
                )
                .expect("frames match the recording size");
            encoder
        })
        .finalize(milliseconds_since_first(
            last.captured_at + LAST_FRAME_DURATION,
        ))
        .expect("animation is encodable");
    fs::write(animation, &*webp_bytes).expect("recordings directory is writable");
}

#[must_use]
pub fn record(demo: &'static Demo, start: Clock) -> RecordedAnimation {
    let config = Config::fallback();
    register_font(&config);
    let margin_pixels = (config.layout.overlay.panel_gap * SCALE_FACTOR).round() as u16;
    let mut recorder = DemoRecorder::start(demo, &config, start);
    let opening = recorder.rest();
    let scripted: Vec<CapturedFrame> = demo
        .script
        .iter()
        .flat_map(|step| recorder.perform(step))
        .collect();
    let closing = recorder.rest();
    let frames: Vec<CapturedFrame> = opening.into_iter().chain(scripted).chain(closing).collect();
    let crop = ContentCrop::around_content(
        &frames,
        FrameSize::of_canvas(),
        recorder.background,
        margin_pixels,
    );
    drop(recorder);
    let animation = Path::new(WORKSPACE_ROOT)
        .join(RECORDINGS_DIRECTORY)
        .join(format!("{}.{RECORDING_EXTENSION}", demo.name));
    encode_animation(&frames, &crop, &animation);
    RecordedAnimation {
        name: demo.name,
        width: crop.logical_width(),
    }
}
