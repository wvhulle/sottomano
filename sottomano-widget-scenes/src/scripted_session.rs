use std::{borrow::Cow, time::Duration};

use iced::{
    Point, Rectangle, Size,
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
    instruction::{Interaction, Key, Keyboard, Mouse, Target},
    program::Program,
    renderer::graphics::text::font_system,
    runtime::{UserInterface, user_interface::Cache},
};
use sottomano_toml_config::Config;
use sottomano_widgets::clock::Clock;

use crate::{
    hovered_widget::AnyHoveredWidget, scenes::Scene, showcase_message::ShowcaseMessage,
    showcase_program::ShowcaseProgram,
};

mod step_performance;

pub const SCALE_FACTOR: f32 = 2.0;
pub const FRAME_INTERVAL: Duration = Duration::from_millis(20);
const EMULATOR_EVENT_CAPACITY: usize = 256;

pub trait FrameWatcher {
    fn frame_advanced(&mut self, rendered_frame: RenderedFrame<'_>);
}

impl FrameWatcher for () {
    fn frame_advanced(&mut self, _rendered_frame: RenderedFrame<'_>) {}
}

pub struct RenderedFrame<'frame> {
    emulator: &'frame mut Emulator<ShowcaseProgram>,
    program: &'frame ShowcaseProgram,
    pub clock: Clock,
}

impl RenderedFrame<'_> {
    pub fn screenshot(&mut self) -> Vec<u8> {
        emulator_screenshot(self.emulator, self.program)
    }
}

fn emulator_screenshot(
    emulator: &mut Emulator<ShowcaseProgram>,
    program: &ShowcaseProgram,
) -> Vec<u8> {
    let theme = emulator
        .theme(program)
        .expect("the showcase program always sets a theme");
    emulator
        .screenshot(program, &theme, SCALE_FACTOR)
        .rgba
        .to_vec()
}

pub struct ScriptedSession<Watcher> {
    scene: &'static Scene,
    pub config: Config,
    program: ShowcaseProgram,
    emulator: Emulator<ShowcaseProgram>,
    events: Receiver<Event<ShowcaseProgram>>,
    pub layout_renderer: iced::Renderer,
    pointer: Point,
    clock: Clock,
    pub canvas: Size,
    pub watcher: Watcher,
}

impl ScriptedSession<()> {
    #[must_use]
    pub fn start(scene: &'static Scene, clock: Clock) -> Self {
        let config = Config::default();
        font_system()
            .write()
            .expect("font system lock is never poisoned")
            .load_font(Cow::Owned(config.theme.font.data.to_vec()));
        let program = ShowcaseProgram::scripted(&config, clock);
        let settings = program.settings();
        let layout_renderer = block_on(iced::Renderer::new(
            settings.default_font,
            settings.default_text_size,
            None,
        ))
        .expect("a headless renderer is available");
        let canvas = scene.backdrop.canvas();
        let (sender, events) = mpsc::channel(EMULATOR_EVENT_CAPACITY);
        let emulator = Emulator::with_preset(
            sender,
            &program,
            Mode::Zen,
            canvas,
            Some(program.preset(scene)),
        );
        let mut session = ScriptedSession {
            scene,
            config,
            program,
            emulator,
            events,
            layout_renderer,
            pointer: step_performance::rest_pointer(canvas),
            clock,
            canvas,
            watcher: (),
        };
        session.settle();
        session.deliver_layout_reports();
        session
    }
}

impl<Watcher: FrameWatcher> ScriptedSession<Watcher> {
    pub fn watched_by<NextWatcher: FrameWatcher>(
        self,
        watcher: NextWatcher,
    ) -> ScriptedSession<NextWatcher> {
        ScriptedSession {
            scene: self.scene,
            config: self.config,
            program: self.program,
            emulator: self.emulator,
            events: self.events,
            layout_renderer: self.layout_renderer,
            pointer: self.pointer,
            clock: self.clock,
            canvas: self.canvas,
            watcher,
        }
    }

    pub fn screenshot(&mut self) -> Vec<u8> {
        emulator_screenshot(&mut self.emulator, &self.program)
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
                    "scene {} cannot {instruction}: its target is not on screen",
                    self.scene.name
                ),
                Some(Event::Ready) => return,
                None => panic!("scene {} lost its emulator", self.scene.name),
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
            &mut self.layout_renderer,
        );
        let wanted = selector.description();
        let mut finder = selector.find_all();
        interface.operate(&self.layout_renderer, &mut black_box(&mut finder));
        if let Outcome::Some(matches) = finder.finish()
            && let Some(last_laid_out) = matches.last()
        {
            assert!(
                last_laid_out.width > 0.0 && last_laid_out.height > 0.0,
                "scene {} points at {wanted}, which is laid out without area: {last_laid_out:?}",
                self.scene.name
            );
            return last_laid_out.center();
        }
        let mut shown = AnyHoveredWidget.find_all();
        interface.operate(&self.layout_renderer, &mut black_box(&mut shown));
        let shown = match shown.finish() {
            Outcome::Some(targets) => targets,
            _ => Vec::new(),
        };
        panic!(
            "scene {} points at {wanted}, which is not on screen; hover areas shown now: {shown:?}",
            self.scene.name
        )
    }

    fn advance_frame(&mut self) {
        self.clock = Clock {
            instant: self.clock.instant + FRAME_INTERVAL,
            wall: self.clock.wall,
        };
        self.emulator
            .update(&self.program, ShowcaseMessage::Frame(self.clock));
        self.deliver_layout_reports();
        self.watcher.frame_advanced(RenderedFrame {
            emulator: &mut self.emulator,
            program: &self.program,
            clock: self.clock,
        });
    }
}
