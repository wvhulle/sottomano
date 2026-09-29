use std::collections::HashMap;

use iced::{
    Element, Rectangle, Task,
    time::Duration,
    widget::{container, mouse_area},
    window::Id,
};
use iced_exwlshell::{
    actions::ActionCallback,
    reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings},
};
use tokio::time::sleep;

use crate::{
    app::Message,
    config::Config,
    wayland_layers::{
        surface::{self, Position, Surface},
        tooltip::{
            HoverTarget, TooltipMessage,
            tooltip_content::TooltipContent,
            tooltip_transition::{Departed, Occupant, Placement, TooltipTransition},
        },
    },
    widgets::{
        anchored_popup::{Rendering, anchored_popup},
        favicon::Favicons,
        tooltip_body::Tooltip,
    },
};

const HIDE_GRACE: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct TooltipHost {
    surfaces: HashMap<String, Id>,
    transition: TooltipTransition,
}

impl TooltipHost {
    pub fn update(&mut self, message: TooltipMessage) -> Task<Message> {
        match message {
            TooltipMessage::Exit { target } => self.exit(&target),
            TooltipMessage::EnterTooltip => {
                self.transition.cancel_leave();
                Task::none()
            }
            TooltipMessage::ExitTooltip => self.leave_after_grace(),
            TooltipMessage::Enter { .. } | TooltipMessage::EnterAnchor { .. } => Task::none(),
        }
    }

    pub fn open(
        &mut self,
        parent: &Surface,
        target: HoverTarget,
        anchor: Rectangle,
        departing: Option<Departed>,
    ) {
        self.transition.cancel_leave();
        if self
            .transition
            .current()
            .is_some_and(|current| current.target == target)
        {
            return;
        }
        self.transition.show(
            Occupant {
                target,
                placement: Placement {
                    anchor,
                    position: parent.position,
                    output: parent.output.clone(),
                },
            },
            departing,
        );
    }

    fn exit(&mut self, target: &HoverTarget) -> Task<Message> {
        if !self
            .transition
            .current()
            .is_some_and(|current| &current.target == target)
        {
            return Task::none();
        }
        self.leave_after_grace()
    }

    pub fn leave_after_grace(&mut self) -> Task<Message> {
        self.transition.request_leave();
        Task::future(sleep(HIDE_GRACE)).map(|()| Message::TooltipHideElapsed)
    }

    pub fn leave_elapsed(&mut self, departing: Option<Departed>) -> Task<Message> {
        if !self.transition.is_leaving() {
            return Task::none();
        }
        self.transition.hide(departing);
        self.clear_input_regions()
    }

    pub fn dismiss(&mut self, departing: Option<Departed>) -> Task<Message> {
        if self.transition.current().is_none() {
            return Task::none();
        }
        self.transition.hide(departing);
        self.clear_input_regions()
    }

    fn clear_input_regions(&self) -> Task<Message> {
        Task::batch(
            self.surfaces()
                .map(|surface| Task::done(input_region(surface, Rectangle::default()))),
        )
    }

    pub fn open_surface(&mut self, output: &str) -> Task<Message> {
        if self.surfaces.contains_key(output) {
            return Task::none();
        }
        let id = Id::unique();
        self.surfaces.insert(output.to_string(), id);
        Task::batch([
            Task::done(Message::NewLayerShell {
                settings: tooltip_settings(output),
                id,
            }),
            Task::done(input_region(id, Rectangle::default())),
        ])
    }

    pub fn surfaces(&self) -> impl Iterator<Item = Id> {
        self.surfaces.values().copied()
    }

    pub fn forget(&mut self, surface: Id) {
        let Some(output) = self.output_of(surface) else {
            return;
        };
        self.forget_output(&output);
    }

    pub fn forget_output(&mut self, output: &str) {
        self.surfaces.remove(output);
        if self
            .transition
            .placement()
            .is_some_and(|placement| placement.output == output)
        {
            self.transition.reset();
        }
    }

    fn output_of(&self, surface: Id) -> Option<String> {
        self.surfaces
            .iter()
            .find(|(_, id)| **id == surface)
            .map(|(output, _)| output.clone())
    }

    fn hosts(&self, surface: Id) -> bool {
        self.surfaces.values().any(|id| *id == surface)
    }

    pub fn target(&self) -> Option<&HoverTarget> {
        self.transition.current().map(|current| &current.target)
    }

    pub fn departure(&self, content: Tooltip) -> Option<Departed> {
        let placement = self.transition.current()?.placement.clone();
        Some(Departed::new(content, placement))
    }

    pub fn is_morphing(&self) -> bool {
        self.transition.is_morphing()
    }

    pub fn view<'a>(
        &self,
        surface: Id,
        config: &'a Config,
        float_offset: f32,
        favicons: &'a Favicons,
        content: impl Fn(&HoverTarget) -> Tooltip,
    ) -> Option<Element<'a, Message>> {
        if !self.hosts(surface) {
            return None;
        }
        let Some(placement) = self
            .transition
            .placement()
            .filter(|placement| self.surfaces.get(&placement.output) == Some(&surface))
        else {
            return Some(anchored_popup(
                Vec::new(),
                Rectangle::default(),
                Position::Top,
                0.0,
                self.transition.seq(),
                config,
                move |bounds| input_region(surface, bounds),
            ));
        };

        let progress = self.transition.progress();
        let mut renderings = Vec::new();
        if let Some(previous) = self.transition.previous() {
            let opacity = (1.0 - 2.0 * progress).max(0.0);
            if opacity > 0.0 {
                renderings.push(Rendering::outgoing(
                    TooltipContent::new(&previous.content, config, favicons)
                        .opacity(opacity)
                        .view(),
                ));
            }
        }
        if let Some(current) = self.transition.current() {
            let opacity = (2.0 * progress - 1.0).max(0.0);
            renderings.push(Rendering::incoming(hoverable_content(
                TooltipContent::new(&content(&current.target), config, favicons)
                    .opacity(opacity)
                    .view(),
            )));
        }
        Some(
            container(anchored_popup(
                renderings,
                placement.anchor,
                placement.position,
                progress,
                self.transition.seq(),
                config,
                move |bounds| input_region(surface, bounds),
            ))
            .padding(float_offset)
            .into(),
        )
    }
}

fn hoverable_content(content: Element<'_, Message>) -> Element<'_, Message> {
    mouse_area(content)
        .on_enter(Message::Tooltip(TooltipMessage::EnterTooltip))
        .on_exit(Message::Tooltip(TooltipMessage::ExitTooltip))
        .into()
}

fn input_region(surface: Id, bounds: Rectangle) -> Message {
    Message::SetInputRegion {
        id: surface,
        callback: ActionCallback::new(move |region| {
            if bounds.width > 0.0 && bounds.height > 0.0 {
                region.add(
                    bounds.x as i32,
                    bounds.y as i32,
                    bounds.width as i32,
                    bounds.height as i32,
                );
            }
        }),
    }
}

fn tooltip_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        ..surface::base_settings(Some(output), "vetrata-tooltip")
    }
}
