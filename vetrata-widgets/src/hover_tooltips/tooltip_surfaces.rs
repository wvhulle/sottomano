use std::collections::HashMap;

use iced::{
    Element, Rectangle, Task,
    border::Radius,
    task::Handle,
    time::Duration,
    widget::{container, mouse_area},
    window::Id,
};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use tokio::time::sleep;
use vetrata_config::{Config, screen_edges::Position};
use vetrata_state::{ShellSettings, niri_rules::StyledSurface};

use crate::{
    hover_tooltips::{
        HoverTarget, PressHints, TooltipMessage,
        tooltip_content::TooltipContent,
        tooltip_transition::{Departed, Occupant, Placement, TooltipTransition},
    },
    layer_surfaces::{PanelEdge, base_settings, input_region},
    shell_message::Message,
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
    seq: u64,
}

#[derive(Default)]
pub struct TooltipHost {
    surfaces: HashMap<String, Id>,
    transition: TooltipTransition,
    pending_switch: Option<PendingSwitch>,
    switch_seq: u64,
    hide_grace: Option<Handle>,
}

impl TooltipHost {
    pub fn update(&mut self, message: TooltipMessage) -> Task<Message> {
        match message {
            TooltipMessage::Exit { target } => self.exit(&target),
            TooltipMessage::EnterTooltip => {
                self.pending_switch = None;
                self.hide_grace = None;
                self.transition.cancel_leave();
                Task::none()
            }
            TooltipMessage::ExitTooltip => self.leave_after_grace(),
            TooltipMessage::PressHintsChanged {
                target,
                press_hints,
            } => {
                self.transition.revise_press_hints(&target, press_hints);
                Task::none()
            }
            TooltipMessage::Enter { .. }
            | TooltipMessage::EnterAnchor { .. }
            | TooltipMessage::SwitchIntentElapsed { .. } => Task::none(),
        }
    }

    pub fn defer_switch(
        &mut self,
        parent: &PanelEdge,
        target: &HoverTarget,
        anchor: Rectangle,
        press_hints: PressHints,
    ) -> Option<Task<Message>> {
        let current = self.transition.current()?;
        if !parent.position.is_vertical() || &current.target == target {
            return None;
        }
        self.transition.cancel_leave();
        self.switch_seq += 1;
        let seq = self.switch_seq;
        self.pending_switch = Some(PendingSwitch {
            parent: parent.clone(),
            target: target.clone(),
            anchor,
            press_hints,
            seq,
        });
        Some(
            Task::future(sleep(SWITCH_INTENT))
                .map(move |()| Message::Tooltip(TooltipMessage::SwitchIntentElapsed { seq })),
        )
    }

    pub fn take_pending_switch(&mut self, seq: u64) -> Option<PendingSwitch> {
        self.pending_switch.take_if(|pending| pending.seq == seq)
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

    fn exit(&mut self, target: &HoverTarget) -> Task<Message> {
        if self
            .pending_switch
            .take_if(|pending| &pending.target == target)
            .is_some()
        {
            return self.leave_after_grace();
        }
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
        let (task, handle) = Task::future(sleep(HIDE_GRACE))
            .map(|()| Message::TooltipHideElapsed)
            .abortable();
        self.hide_grace = Some(handle.abort_on_drop());
        task
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
    ) -> Option<Element<'a, Message>> {
        if !self.hosts(surface) {
            return None;
        }
        let corner_radius = Radius::from(shell_settings.overlays.corner_radius_px);
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
                TooltipContent::new(&previous.content, config, corner_radius, assets).view(),
            ));
        }
        if let Some(current) = self.transition.current() {
            renderings.push(Rendering::incoming(hoverable_content(
                TooltipContent::new(
                    &content(&current.target).with_press_hints(current.press_hints),
                    config,
                    corner_radius,
                    assets,
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

fn hoverable_content(content: Element<'_, Message>) -> Element<'_, Message> {
    mouse_area(content)
        .on_enter(Message::Tooltip(TooltipMessage::EnterTooltip))
        .on_exit(Message::Tooltip(TooltipMessage::ExitTooltip))
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
