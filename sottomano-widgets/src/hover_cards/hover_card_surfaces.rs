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
    group_menus::cascading_layout::cascade_row::{CascadeOrientation, cascade_row},
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, PopupMessage,
        anchored_popup::{PopupFraming, PopupMorph, Rendering, anchored_popup},
        hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_content::HoverCardContent,
        hover_card_transition::{
            Departed, DepartedContent, HoverCardTransition, Occupant, Placement, ShownTarget,
        },
    },
    layer_surfaces::{PanelEdge, SurfaceRequest, base_settings, input_regions},
    widget_groups::niri_window_group::niri_kdl_rules::StyledSurface,
    widgets::{shell_layer_appearance::LayerAppearance, window_icon::WindowAssets},
};

pub const HIDE_GRACE: Duration = Duration::from_millis(250);
const SWITCH_INTENT: Duration = Duration::from_millis(180);

pub struct PendingSwitch {
    pub parent: PanelEdge,
    pub target: HoverTarget,
    pub anchor: Rectangle,
    pub announcement: HoverAnnouncement,
    _intent_timer: task::Handle,
}

#[derive(Default)]
pub struct HoverCardHost {
    surfaces: HashMap<String, Id>,
    transition: HoverCardTransition,
    pending_switch: Option<PendingSwitch>,
    hide_due: Option<Instant>,
    pointer_over_hover_card: bool,
}

impl HoverCardHost {
    pub fn update(&mut self, message: HoverCardMessage, now: Instant) -> Option<Instant> {
        match message {
            HoverCardMessage::Exit { target } => self.exit(&target, now),
            HoverCardMessage::EnterHoverCard => {
                self.pointer_over_hover_card = true;
                self.pending_switch = None;
                self.hide_due = None;
                self.transition.cancel_leave();
                None
            }
            HoverCardMessage::ExitHoverCard => {
                self.pointer_over_hover_card = false;
                Some(self.leave_after_grace(now))
            }
            HoverCardMessage::AnnouncementChanged {
                target,
                announcement,
            } => {
                self.transition.revise_announcement(&target, announcement);
                None
            }
            HoverCardMessage::Enter { .. }
            | HoverCardMessage::EnterAnchor { .. }
            | HoverCardMessage::SwitchIntentElapsed => None,
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
        announcement: HoverAnnouncement,
    ) -> Option<Task<HoverCardMessage>> {
        let current = self.transition.current()?;
        if !parent.position.is_vertical() || &current.target == target {
            return None;
        }
        self.transition.cancel_leave();
        let (intent_elapsed, handle) = Task::future(sleep(SWITCH_INTENT))
            .map(|()| HoverCardMessage::SwitchIntentElapsed)
            .abortable();
        self.pending_switch = Some(PendingSwitch {
            parent: parent.clone(),
            target: target.clone(),
            anchor,
            announcement,
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
        announcement: HoverAnnouncement,
        now: Instant,
    ) {
        self.pending_switch = None;
        self.pointer_over_hover_card = false;
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
                announcement,
                placement: Placement {
                    anchor,
                    position: parent.position,
                    output: parent.output.clone(),
                },
            },
            now,
        );
    }

    fn exit(&mut self, target: &HoverTarget, now: Instant) -> Option<Instant> {
        if self
            .pending_switch
            .take_if(|pending| &pending.target == target)
            .is_some()
        {
            return Some(self.leave_after_grace(now));
        }
        if self.pointer_over_hover_card
            || !self
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
            self.pointer_over_hover_card = false;
            self.transition.hide(departing, now);
        }
    }

    pub fn dismiss(&mut self, departing: Option<Departed>, now: Instant) {
        self.pointer_over_hover_card = false;
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
                settings: hover_card_settings(output),
                surface: id,
            }),
            Task::done(input_regions(id, Vec::new())),
        ])
    }

    #[must_use]
    pub fn placement(&self) -> Option<&Placement> {
        self.transition.placement()
    }

    #[must_use]
    pub fn is_morphing(&self, now: Instant, morph: Duration) -> bool {
        self.transition.morph_progress(now, morph) < 1.0
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
        let departed_content = match content.with_announcement(current.announcement.clone()) {
            HoverContent::Described(hover_card) => DepartedContent::Described(hover_card),
            HoverContent::Interactive { .. } | HoverContent::GroupMenu { .. } => {
                DepartedContent::Interactive
            }
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
        surface_style: HoverCardSurfaceStyle,
        assets: WindowAssets<'a>,
        content_of: impl Fn(&HoverTarget) -> HoverContent<'a, Message>,
        clock: Clock,
    ) -> Option<Element<'a, Message>> {
        let describe = move |hover_card: &HoverCard| -> Element<'a, Message> {
            Element::from(HoverCardContent::new(hover_card, config, assets, clock).view())
                .map(Message::from)
        };
        let render = |content: HoverContent<'a, Message>| match content {
            HoverContent::Described(hover_card) => Rendered::framed(describe(&hover_card)),
            HoverContent::Interactive { described, panel } => {
                if described.is_empty() {
                    Rendered::framed(panel)
                } else {
                    Rendered::framed(
                        HoverCardContent::new(&described, config, assets, clock)
                            .view_enclosing(panel),
                    )
                }
            }
            HoverContent::GroupMenu {
                primary,
                satellites,
                orientation,
            } => Rendered {
                element: cascade_row(primary, satellites, orientation),
                framing: PopupFraming::LeadingSurface(orientation),
            },
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
                PopupMorph {
                    progress: 1.0,
                    shown: ShownTarget::Nothing,
                },
                config,
                surface_style.overlay_appearance,
            ));
        };

        let mut renderings = Vec::new();
        if let Some(previous) = self.transition.previous() {
            let departed = match &previous.content {
                DepartedContent::Described(hover_card) => {
                    HoverContent::Described(hover_card.clone())
                }
                DepartedContent::Interactive => content_of(&previous.target),
            };
            let Rendered { element, framing } = render(departed);
            renderings.push(Rendering::outgoing(element).framing(framing));
        }
        if let Some(current) = self.transition.current() {
            let Rendered { element, framing } =
                render(content_of(&current.target).with_announcement(current.announcement.clone()));
            renderings.push(Rendering::incoming(hoverable_content(element)).framing(framing));
        }
        Some(
            container(anchored_popup(
                renderings,
                placement.anchor,
                placement.position,
                PopupMorph {
                    progress: self
                        .transition
                        .morph_progress(clock.instant, config.tooltips.popup_morph),
                    shown: self.transition.shown_target(),
                },
                config,
                surface_style.overlay_appearance,
            ))
            .padding(surface_style.float_offset_px)
            .into(),
        )
    }
}

pub enum HoverContent<'a, Message> {
    Described(HoverCard),
    Interactive {
        described: HoverCard,
        panel: Element<'a, Message>,
    },
    GroupMenu {
        primary: Element<'a, Message>,
        satellites: Vec<Element<'a, Message>>,
        orientation: CascadeOrientation,
    },
}

struct Rendered<'a, Message> {
    element: Element<'a, Message>,
    framing: PopupFraming,
}

impl<'a, Message> Rendered<'a, Message> {
    fn framed(element: Element<'a, Message>) -> Self {
        Self {
            element,
            framing: PopupFraming::WholeContent,
        }
    }
}

impl<Message> HoverContent<'_, Message> {
    #[must_use]
    pub fn with_announcement(self, announcement: HoverAnnouncement) -> Self {
        match self {
            HoverContent::Described(hover_card) => {
                HoverContent::Described(hover_card.with_announcement(announcement))
            }
            HoverContent::Interactive { described, panel } => HoverContent::Interactive {
                described: described.with_announcement(announcement),
                panel,
            },
            cascade @ HoverContent::GroupMenu { .. } => cascade,
        }
    }

    #[must_use]
    pub fn shows_anything(&self, announcement: &HoverAnnouncement) -> bool {
        match self {
            HoverContent::Described(hover_card) => {
                !hover_card.is_empty() || !announcement.is_empty()
            }
            HoverContent::Interactive { .. } | HoverContent::GroupMenu { .. } => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HoverCardSurfaceStyle {
    pub overlay_appearance: LayerAppearance,
    pub float_offset_px: f32,
}

fn hoverable_content<'a, Message: From<PopupMessage> + Clone + 'a>(
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    mouse_area(content)
        .on_enter(Message::from(PopupMessage::HoverCard(
            HoverCardMessage::EnterHoverCard,
        )))
        .on_exit(Message::from(PopupMessage::HoverCard(
            HoverCardMessage::ExitHoverCard,
        )))
        .into()
}

fn hover_card_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        ..base_settings(Some(output), StyledSurface::HoverCard.namespace())
    }
}
