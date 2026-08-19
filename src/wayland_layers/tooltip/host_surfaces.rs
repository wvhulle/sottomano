use std::collections::HashMap;

use iced::{Element, Rectangle, Task, time::Duration, widget::Space, window::Id};
use iced_layershell::reexport::{Anchor, Layer, NewLayerShellSettings};
use tokio::time::sleep;

use crate::{
    app::Message,
    config::Config,
    data::source::{Tooltip, niri::OutputInfo},
    wayland_layers::{
        surface::{self, Position, Surface},
        tooltip::{
            HoverTarget, TooltipMessage,
            tooltip_content::render,
            tooltip_placement::{Rendering, morphing_tooltip},
            tooltip_transition::{Departed, Occupant, Placement, TooltipTransition},
        },
    },
};

const HIDE_GRACE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct MenuAnchor {
    pub anchor: Rectangle,
    pub output: String,
    pub output_width: Option<i32>,
    pub position: Position,
}

#[derive(Default)]
pub struct TooltipHost {
    surfaces: HashMap<String, Id>,
    transition: TooltipTransition,
}

impl TooltipHost {
    pub fn update(&mut self, message: TooltipMessage) -> Task<Message> {
        match message {
            TooltipMessage::Exit { target } => self.exit(&target),
            TooltipMessage::Enter { .. } | TooltipMessage::EnterAnchor { .. } => Task::none(),
        }
    }

    pub fn open(
        &mut self,
        parent: &Surface,
        target: HoverTarget,
        anchor: Rectangle,
        outputs: &HashMap<String, OutputInfo>,
        departing: Option<Departed>,
    ) -> Task<Message> {
        self.transition.cancel_leave();
        if self
            .transition
            .current()
            .is_some_and(|current| current.target == target)
        {
            return Task::none();
        }
        self.transition.show(
            Occupant {
                target,
                placement: Placement {
                    anchor,
                    position: parent.position,
                    output: parent.output.clone(),
                },
                output_width: outputs.get(&parent.output).map(|info| info.width),
            },
            departing,
        );
        self.ensure_surface(&parent.output)
    }

    fn exit(&mut self, target: &HoverTarget) -> Task<Message> {
        if !self
            .transition
            .current()
            .is_some_and(|current| &current.target == target)
        {
            return Task::none();
        }
        self.transition.request_leave();
        Task::future(sleep(HIDE_GRACE)).map(|()| Message::TooltipHideElapsed)
    }

    pub fn leave_elapsed(&mut self, departing: Option<Departed>) {
        if self.transition.is_leaving() {
            self.transition.hide(departing);
        }
    }

    pub fn dismiss(&mut self, departing: Option<Departed>) {
        if self.transition.current().is_some() {
            self.transition.hide(departing);
        }
    }

    fn ensure_surface(&mut self, output: &str) -> Task<Message> {
        if self.surfaces.contains_key(output) {
            return Task::none();
        }
        let id = Id::unique();
        self.surfaces.insert(output.to_string(), id);
        Task::done(Message::NewLayerShell {
            settings: tooltip_settings(output),
            id,
        })
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

    pub fn menu_anchor(&self) -> Option<MenuAnchor> {
        let current = self.transition.current()?;
        matches!(current.target, HoverTarget::Tray(_)).then(|| MenuAnchor {
            anchor: current.placement.anchor,
            output: current.placement.output.clone(),
            output_width: current.output_width,
            position: current.placement.position,
        })
    }

    pub fn view<'a>(
        &self,
        surface: Id,
        config: &'a Config,
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
            return Some(Space::new().into());
        };

        let progress = self.transition.progress();
        let mut renderings = Vec::new();
        if let Some(previous) = self.transition.previous() {
            let alpha = (1.0 - 2.0 * progress).max(0.0);
            if alpha > 0.0 {
                renderings.push(Rendering::outgoing(render(
                    &previous.content,
                    config,
                    alpha,
                )));
            }
        }
        if let Some(current) = self.transition.current() {
            let alpha = (2.0 * progress - 1.0).max(0.0);
            renderings.push(Rendering::incoming(render(
                &content(&current.target),
                config,
                alpha,
            )));
        }
        Some(morphing_tooltip(
            renderings,
            placement.anchor,
            placement.position,
            progress,
            self.transition.seq(),
            config,
        ))
    }
}

fn tooltip_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, 0)),
        exclusive_zone: Some(0),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        events_transparent: true,
        ..surface::base_settings(Some(output), "vetrata-tooltip")
    }
}
