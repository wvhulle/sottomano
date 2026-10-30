use std::collections::HashMap;

use iced::{
    Element, Rectangle, Task, task,
    time::{Duration, Instant},
    widget::{container, mouse_area},
    window::Id,
};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use sottomano_config::{Config, screen_edges::Position};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    hover_tooltips::{
        HoverTarget, PopupMessage, PressHints, TooltipMessage,
        tooltip_content::TooltipContent,
        tooltip_transition::{Departed, DepartedContent, Occupant, Placement, TooltipTransition},
    },
    layer_surfaces::{PanelEdge, SurfaceRequest, base_settings, input_regions},
    widget_groups::niri_window_group::niri_kdl_rules::StyledSurface,
    widgets::{
        anchored_popup::{Rendering, anchored_popup},
        shell_layer_appearance::LayerAppearance,
        tooltip_body::tooltip_source_delivery::Tooltip,
        window_icon::WindowAssets,
    },
};

pub const HIDE_GRACE: Duration = Duration::from_millis(250);
const SWITCH_INTENT: Duration = Duration::from_millis(180);

pub struct PendingSwitch {
    pub parent: PanelEdge,
    pub target: HoverTarget,
    pub anchor: Rectangle,
    pub press_hints: PressHints,
    _intent_timer: task::Handle,
}

#[derive(Default)]
pub struct TooltipHost {
    surfaces: HashMap<String, Id>,
    transition: TooltipTransition,
    pending_switch: Option<PendingSwitch>,
    hide_due: Option<Instant>,
}

impl TooltipHost {
    pub fn update(&mut self, message: TooltipMessage, now: Instant) -> Option<Instant> {
        match message {
            TooltipMessage::Exit { target } => self.exit(&target, now),
            TooltipMessage::EnterTooltip => {
                self.pending_switch = None;
                self.hide_due = None;
                self.transition.cancel_leave();
                None
            }
            TooltipMessage::ExitTooltip => Some(self.leave_after_grace(now)),
            TooltipMessage::PressHintsChanged {
                target,
                press_hints,
            } => {
                self.transition.revise_press_hints(&target, press_hints);
                None
            }
            TooltipMessage::Enter { .. }
            | TooltipMessage::EnterAnchor { .. }
            | TooltipMessage::SwitchIntentElapsed => None,
        }
    }

    #[must_use]
    pub fn hide_due(&self) -> Option<Instant> {
        self.hide_due
    }

    pub fn defer_switch(
        &mut self,
        parent: &PanelEdge,
        target: &HoverTarget,
        anchor: Rectangle,
        press_hints: PressHints,
    ) -> Option<Task<TooltipMessage>> {
        let current = self.transition.current()?;
        if !parent.position.is_vertical() || &current.target == target {
            return None;
        }
        self.transition.cancel_leave();
        let (intent_elapsed, handle) = Task::future(sleep(SWITCH_INTENT))
            .map(|()| TooltipMessage::SwitchIntentElapsed)
            .abortable();
        self.pending_switch = Some(PendingSwitch {
            parent: parent.clone(),
            target: target.clone(),
            anchor,
            press_hints,
            _intent_timer: handle.abort_on_drop(),
        });
        Some(intent_elapsed)
    }

    pub fn take_pending_switch(&mut self) -> Option<PendingSwitch> {
        self.pending_switch.take()
    }

    pub fn open(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        anchor: Rectangle,
        press_hints: PressHints,
    ) {
        self.pending_switch = None;
        self.transition.cancel_leave();
        if self
            .transition
            .current()
            .is_some_and(|current| current.target == target)
        {
            return;
        }
        self.transition.show(Occupant {
            target,
            press_hints,
            placement: Placement {
                anchor,
                position: parent.position,
                output: parent.output.clone(),
            },
        });
    }

    fn exit(&mut self, target: &HoverTarget, now: Instant) -> Option<Instant> {
        if self
            .pending_switch
            .take_if(|pending| &pending.target == target)
            .is_some()
        {
            return Some(self.leave_after_grace(now));
        }
        if !self
            .transition
            .current()
            .is_some_and(|current| &current.target == target)
        {
            return None;
        }
        Some(self.leave_after_grace(now))
    }

    pub fn leave_after_grace(&mut self, now: Instant) -> Instant {
        self.transition.request_leave();
        let due = now + HIDE_GRACE;
        self.hide_due = Some(due);
        due
    }

    pub fn leave_elapsed(&mut self, now: Instant, departing: Option<Departed>) {
        if self.transition.is_leaving() && self.hide_due.take_if(|due| *due <= now).is_some() {
            self.transition.hide(departing, now);
        }
    }

    pub fn dismiss(&mut self, departing: Option<Departed>, now: Instant) {
        if self.transition.current().is_some() {
            self.transition.hide(departing, now);
        }
    }

    pub fn open_surface(&mut self, output: &str) -> Task<SurfaceRequest> {
        if self.surfaces.contains_key(output) {
            return Task::none();
        }
        let id = Id::unique();
        self.surfaces.insert(output.to_string(), id);
        Task::batch([
            Task::done(SurfaceRequest::Open {
                settings: tooltip_settings(output),
                surface: id,
            }),
            Task::done(input_regions(id, Vec::new())),
        ])
    }

    #[must_use]
    pub fn is_hiding(&self) -> bool {
        self.transition.hiding_since().is_some()
    }

    pub fn reconcile_surfaces(&mut self, now: Instant, fade: Duration) -> Task<SurfaceRequest> {
        if self
            .transition
            .hiding_since()
            .is_some_and(|since| now >= since + fade)
        {
            self.transition.reset();
        }
        let wanted_output = self
            .transition
            .placement()
            .map(|placement| placement.output.clone());
        let stale_surfaces: Vec<(String, Id)> = self
            .surfaces
            .iter()
            .filter(|(output, _)| wanted_output.as_ref() != Some(*output))
            .map(|(output, surface)| (output.clone(), *surface))
            .collect();
        let closes = stale_surfaces.into_iter().map(|(output, surface)| {
            self.surfaces.remove(&output);
            Task::done(SurfaceRequest::Close { surface })
        });
        let closes: Vec<Task<SurfaceRequest>> = closes.collect();
        let open = wanted_output.map_or_else(Task::none, |output| self.open_surface(&output));
        Task::batch(closes.into_iter().chain([open]))
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

    #[must_use]
    pub fn surface_on(&self, output: &str) -> Option<Id> {
        self.surfaces.get(output).copied()
    }

    fn hosts(&self, surface: Id) -> bool {
        self.surfaces.values().any(|id| *id == surface)
    }

    #[must_use]
    pub fn target(&self) -> Option<&HoverTarget> {
        self.transition.current().map(|current| &current.target)
    }
    #[must_use]
    pub fn departure<Message>(&self, content: HoverContent<'_, Message>) -> Option<Departed> {
        let current = self.transition.current()?;
        let departed_content = match content.with_press_hints(current.press_hints) {
            HoverContent::Described(tooltip) => DepartedContent::Described(tooltip),
            HoverContent::Interactive { .. } => DepartedContent::Interactive,
        };
        Some(Departed::new(
            current.target.clone(),
            departed_content,
            current.placement.clone(),
        ))
    }

    pub fn view<'a, Message: From<PopupMessage> + Clone + 'a>(
        &self,
        surface: Id,
        config: &'a Config,
        surface_style: TooltipSurfaceStyle,
        assets: WindowAssets<'a>,
        content_of: impl Fn(&HoverTarget) -> HoverContent<'a, Message>,
        clock: Clock,
    ) -> Option<Element<'a, Message>> {
        let describe = move |tooltip: &Tooltip| -> Element<'a, Message> {
            Element::from(TooltipContent::new(tooltip, config, assets, clock).view())
                .map(Message::from)
        };
        let render = |content: HoverContent<'a, Message>| match content {
            HoverContent::Described(tooltip) => describe(&tooltip),
            HoverContent::Interactive { described, panel } => {
                if described.is_empty() {
                    panel
                } else {
                    TooltipContent::new(&described, config, assets, clock).view_enclosing(panel)
                }
            }
        };
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
                self.transition.seq(),
                config,
                surface_style.overlay_appearance,
            ));
        };

        let mut renderings = Vec::new();
        if let Some(previous) = self.transition.previous() {
            let departed = match &previous.content {
                DepartedContent::Described(tooltip) => HoverContent::Described(tooltip.clone()),
                DepartedContent::Interactive => content_of(&previous.target),
            };
            renderings.push(Rendering::outgoing(render(departed)));
        }
        if let Some(current) = self.transition.current() {
            renderings.push(Rendering::incoming(hoverable_content(render(
                content_of(&current.target).with_press_hints(current.press_hints),
            ))));
        }
        Some(
            container(anchored_popup(
                renderings,
                placement.anchor,
                placement.position,
                self.transition.seq(),
                config,
                surface_style.overlay_appearance,
            ))
            .padding(surface_style.float_offset_px)
            .into(),
        )
    }
}

pub enum HoverContent<'a, Message> {
    Described(Tooltip),
    Interactive {
        described: Tooltip,
        panel: Element<'a, Message>,
    },
}

impl<Message> HoverContent<'_, Message> {
    #[must_use]
    pub fn with_press_hints(self, press_hints: PressHints) -> Self {
        match self {
            HoverContent::Described(tooltip) => {
                HoverContent::Described(tooltip.with_press_hints(press_hints))
            }
            HoverContent::Interactive { described, panel } => HoverContent::Interactive {
                described: described.with_press_hints(press_hints),
                panel,
            },
        }
    }

    #[must_use]
    pub fn shows_anything(&self, press_hints: PressHints) -> bool {
        match self {
            HoverContent::Described(tooltip) => !tooltip.is_empty() || !press_hints.is_empty(),
            HoverContent::Interactive { .. } => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TooltipSurfaceStyle {
    pub overlay_appearance: LayerAppearance,
    pub float_offset_px: f32,
}

fn hoverable_content<'a, Message: From<PopupMessage> + Clone + 'a>(
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    mouse_area(content)
        .on_enter(Message::from(PopupMessage::Tooltip(
            TooltipMessage::EnterTooltip,
        )))
        .on_exit(Message::from(PopupMessage::Tooltip(
            TooltipMessage::ExitTooltip,
        )))
        .into()
}

fn tooltip_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        ..base_settings(Some(output), StyledSurface::Tooltip.namespace())
    }
}
