use std::time::Duration;

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
    emulator::{Event, Mode},
    instruction::{Interaction, Key, Keyboard, Mouse, Target},
    runtime::{UserInterface, user_interface::Cache},
};
use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock,
    group_indicators::indicator_group_identity::IndicatorGroupIdentity,
    group_menus::group_menu_state::{GroupMenuPin, PanelPin, PanelPinDuration},
    hover_cards::HoverTarget,
};

use crate::{
    animation_recording::{
        FRAME_INTERVAL, SCALE_FACTOR,
        frame_content_crop::{CapturedFrame, FrameContent, FrameSize},
    },
    demo_script::{Demo, GroupShown, Step},
    hovered_widget::{AnyHoveredWidget, HoveredWidget, MenuPinWidget},
    showcase_message::ShowcaseMessage,
    showcase_program::{HOVER_FETCH_LATENCY, ShowcaseProgram},
};

const REST_HOLD: Duration = Duration::from_millis(250);
const HOVER_HOLD: Duration = Duration::from_millis(1600);
const PRESS_LEAD: Duration = Duration::from_millis(150);
const PRESS_HOLD: Duration = Duration::from_millis(900);
const END_STATE_HOLD: Duration = Duration::from_millis(2000);
const DRAG_MOVES: u16 = 16;
const DRAG_MOVE_HOLD: Duration = Duration::from_millis(40);
const EMULATOR_EVENT_CAPACITY: usize = 256;

fn rest_pointer(canvas: Size) -> Point {
    Point::new(canvas.width - 1.0, canvas.height / 2.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameCapture {
    Screenshots,
    Rehearsal,
}

pub(super) struct DemoRecorder<'session> {
    demo: &'static Demo,
    frame_capture: FrameCapture,
    config: &'session Config,
    program: ShowcaseProgram,
    emulator: Emulator<ShowcaseProgram>,
    events: Receiver<Event<ShowcaseProgram>>,
    layout_renderer: &'session mut iced::Renderer,
    pointer: Point,
    pub(super) background: [u8; 4],
    clock: Clock,
    canvas: Size,
}

impl<'session> DemoRecorder<'session> {
    pub(super) fn start(
        demo: &'static Demo,
        config: &'session Config,
        clock: Clock,
        layout_renderer: &'session mut iced::Renderer,
        frame_capture: FrameCapture,
    ) -> Self {
        let canvas = demo.backdrop.canvas();
        let program = ShowcaseProgram::scripted(config, clock);
        let (sender, events) = mpsc::channel(EMULATOR_EVENT_CAPACITY);
        let emulator = Emulator::with_preset(
            sender,
            &program,
            Mode::Zen,
            canvas,
            Some(program.preset(demo)),
        );
        let mut recorder = DemoRecorder {
            demo,
            frame_capture,
            config,
            program,
            emulator,
            events,
            layout_renderer,
            pointer: rest_pointer(canvas),
            background: [0; 4],
            clock,
            canvas,
        };
        recorder.settle();
        recorder.deliver_layout_reports();
        if frame_capture == FrameCapture::Screenshots {
            recorder.background = recorder.top_right_pixel();
        }
        recorder
    }

    fn top_right_pixel(&mut self) -> [u8; 4] {
        let top_right = (usize::from(FrameSize::from(self.canvas).width) - 1) * 4;
        self.screenshot()[top_right..top_right + 4]
            .try_into()
            .expect("a pixel has four channels")
    }

    fn deliver_layout_reports(&mut self) {
        self.emulator.run(
            &self.program,
            Instruction::Interact(Interaction::Keyboard(Keyboard::Release(Key::Tab))),
        );
        self.settle();
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
            self.canvas,
            Cache::default(),
            self.layout_renderer,
        );
        let wanted = selector.description();
        let mut finder = selector.find_all();
        interface.operate(self.layout_renderer, &mut black_box(&mut finder));
        if let Outcome::Some(matches) = finder.finish()
            && let Some(last_laid_out) = matches.last()
        {
            assert!(
                last_laid_out.width > 0.0 && last_laid_out.height > 0.0,
                "demo {} points at {wanted}, which is laid out without area: {last_laid_out:?}",
                self.demo.name
            );
            return last_laid_out.center();
        }
        let mut shown = AnyHoveredWidget.find_all();
        interface.operate(self.layout_renderer, &mut black_box(&mut shown));
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

    pub(super) fn rest(&mut self) -> Vec<CapturedFrame> {
        self.point_at(rest_pointer(self.canvas));
        self.capture_during(REST_HOLD)
    }

    pub(super) fn hold_end_state(&mut self) -> Vec<CapturedFrame> {
        self.capture_during(END_STATE_HOLD)
    }

    pub(super) fn press_away(&mut self) -> Vec<CapturedFrame> {
        self.point_at(rest_pointer(self.canvas));
        self.click();
        self.capture_during(REST_HOLD)
    }

    pub(super) fn perform(&mut self, step: &Step) -> Vec<CapturedFrame> {
        match step {
            Step::Hover(target) => {
                let pointer = self.locate(HoveredWidget {
                    hover_target: target.clone(),
                });
                self.point_at(pointer);
                self.capture_during(self.hover_hold_for(target))
            }
            Step::Press(target) => {
                let pointer = self.locate(HoveredWidget {
                    hover_target: target.clone(),
                });
                self.press_at(pointer)
            }
            Step::HoverFace(group) => self.perform(&Step::Hover(self.face_target(group))),
            Step::PressFace(group) => self.perform(&Step::Press(self.face_target(group))),
            Step::PressPin(entry) => {
                let pointer = self.locate(MenuPinWidget {
                    menu_pin: entry.clone(),
                });
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
                self.point_at(rest_pointer(self.canvas));
                self.capture_during(*hold)
            }
            Step::DragFace { group, by } => {
                let grabbed = self.locate(HoveredWidget {
                    hover_target: self.face_target(group),
                });
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

    fn face_target(&self, group: &IndicatorGroupIdentity) -> HoverTarget {
        match &self.demo.shown {
            GroupShown::PinnedFrom {
                menu,
                group: pinned_group,
            } if pinned_group == group => HoverTarget::PinnedOnPanel(Box::new(PanelPin {
                menu: *menu,
                entry: GroupMenuPin::Group(group.clone()),
                duration: PanelPinDuration::Permanent,
            })),
            _ => HoverTarget::GroupHeadline(Box::new(group.clone())),
        }
    }

    fn hover_hold_for(&self, target: &HoverTarget) -> Duration {
        let fetch_replies =
            (self.demo.group)(self.clock).start_hovered_fetch(target, self.config, self.clock);
        if fetch_replies.is_empty() {
            HOVER_HOLD
        } else {
            HOVER_FETCH_LATENCY + HOVER_HOLD
        }
    }

    fn capture_during(&mut self, hold: Duration) -> Vec<CapturedFrame> {
        let frame_count = hold.as_millis() / FRAME_INTERVAL.as_millis();
        if self.frame_capture == FrameCapture::Rehearsal {
            (0..frame_count).for_each(|_| self.advance_frame());
            return Vec::new();
        }
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

    fn advance_frame(&mut self) {
        self.clock = self.clock.advanced(FRAME_INTERVAL);
        self.emulator
            .update(&self.program, ShowcaseMessage::Frame(self.clock));
        self.deliver_layout_reports();
    }

    fn capture(&mut self) -> CapturedFrame {
        self.advance_frame();
        let rgba = self.screenshot();
        CapturedFrame {
            content: FrameContent::differing_from(
                self.background,
                rgba,
                FrameSize::from(self.canvas),
            ),
            captured_at: self.clock.instant,
        }
    }
}
