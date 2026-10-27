use std::collections::HashMap;

use iced::{
    Element, Rectangle, Task, task,
    time::{Duration, Instant},
    widget::{container, mouse_area},
    window::Id,
};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use sottomano_config::{Config, screen_edges::Position};
use sottomano_state::{ShellSettings, niri_rules::StyledSurface};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    hover_tooltips::{
        HoverTarget, PopupMessage, PressHints, TooltipMessage,
        tooltip_content::TooltipContent,
        tooltip_transition::{Departed, Occupant, Placement, TooltipTransition},
    },
    layer_surfaces::{PanelEdge, SurfaceRequest, base_settings, input_regions},
    widgets::{
        anchored_popup::{Rendering, anchored_popup},
        tooltip_body::Tooltip,
        window_icon::WindowAssets,
    },
};

const HIDE_GRACE: Duration = Duration::from_millis(250);
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
            self.transition.hide(departing);
        }
    }

    pub fn dismiss(&mut self, departing: Option<Departed>) {
        if self.transition.current().is_some() {
            self.transition.hide(departing);
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
    pub fn seq(&self) -> u64 {
        self.transition.seq()
    }

    #[must_use]
    pub fn surfaces_to_redraw(&self, seq_before_update: u64) -> Vec<Id> {
        if self.transition.seq() != seq_before_update {
            return self.surfaces.values().copied().collect();
        }
        self.transition
            .placement()
            .and_then(|placement| self.surfaces.get(&placement.output))
            .copied()
            .into_iter()
            .collect()
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
    pub fn departure(&self, content: Tooltip) -> Option<Departed> {
        let current = self.transition.current()?;
        Some(Departed::new(
            content.with_press_hints(current.press_hints),
            current.placement.clone(),
        ))
    }

    pub fn view<'a>(
        &self,
        surface: Id,
        config: &'a Config,
        shell_settings: &ShellSettings,
        assets: WindowAssets<'a>,
        content: impl Fn(&HoverTarget) -> Tooltip,
        clock: Clock,
    ) -> Option<Element<'a, PopupMessage>> {
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
                shell_settings,
            ));
        };

        let mut renderings = Vec::new();
        if let Some(previous) = self.transition.previous() {
            renderings.push(Rendering::outgoing(
                TooltipContent::new(&previous.content, config, assets, clock).view(),
            ));
        }
        if let Some(current) = self.transition.current() {
            renderings.push(Rendering::incoming(hoverable_content(
                TooltipContent::new(
                    &content(&current.target).with_press_hints(current.press_hints),
                    config,
                    assets,
                    clock,
                )
                .view(),
            )));
        }
        Some(
            container(anchored_popup(
                renderings,
                placement.anchor,
                placement.position,
                self.transition.seq(),
                config,
                shell_settings,
            ))
            .padding(shell_settings.layout.float_offset_px)
            .into(),
        )
    }
}

fn hoverable_content(content: Element<'_, PopupMessage>) -> Element<'_, PopupMessage> {
    mouse_area(content)
        .on_enter(PopupMessage::Tooltip(TooltipMessage::EnterTooltip))
        .on_exit(PopupMessage::Tooltip(TooltipMessage::ExitTooltip))
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
