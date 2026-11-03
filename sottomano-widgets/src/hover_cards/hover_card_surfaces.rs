use std::collections::{BTreeSet, HashMap};

use iced::{
    Element, Rectangle, Task, task,
    time::{Duration, Instant},
    widget::{container, mouse_area},
    window::Id,
};
use iced_exwlshell::reexport::{
    Anchor, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings,
};
use sottomano_config::{Config, screen_edges::Position};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    group_menus::cascading_layout::cascade_row::{CascadeOrientation, cascade_row},
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, MenuHandleGesture, PopupMessage,
        anchored_popup::{
            OutsidePress, PopupAppearances, PopupFraming, PopupMorph, Rendering, anchored_popup,
        },
        hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_content::HoverCardContent,
        hover_card_transition::{
            CardRetention, Departed, DepartedContent, HoverCardTransition, Occupant, Placement,
            ShownTarget,
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
    keyboard_grab: Option<Id>,
    menu_handles: Vec<PanelMenuHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHandleResponse {
    RememberAnchor,
    CloseMenu,
    OpenMenu,
    Nothing,
}

struct PanelMenuHandle {
    output: String,
    target: HoverTarget,
    anchor: Rectangle,
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
            | HoverCardMessage::DismissHeldOpen
            | HoverCardMessage::MenuHandle { .. }
            | HoverCardMessage::MenuHandleAnchor { .. }
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
        self.occupy(
            parent,
            target,
            anchor,
            announcement,
            CardRetention::UntilPointerLeaves,
            now,
        );
    }

    pub fn open_until_dismissed(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        anchor: Rectangle,
        announcement: HoverAnnouncement,
        now: Instant,
    ) {
        self.occupy(
            parent,
            target,
            anchor,
            announcement,
            CardRetention::UntilDismissed,
            now,
        );
    }

    #[must_use]
    pub fn response_to_menu_handle(
        &self,
        gesture: MenuHandleGesture,
        target: &HoverTarget,
        output: &str,
    ) -> MenuHandleResponse {
        let is_open_menu = self.target() == Some(target);
        let held_on_same_output = self.is_held_open()
            && self
                .placement()
                .is_some_and(|placement| placement.output == output);
        match gesture {
            MenuHandleGesture::LaidOut => MenuHandleResponse::RememberAnchor,
            MenuHandleGesture::Pressed if is_open_menu => MenuHandleResponse::CloseMenu,
            MenuHandleGesture::Pressed => MenuHandleResponse::OpenMenu,
            MenuHandleGesture::Hovered if held_on_same_output && !is_open_menu => {
                MenuHandleResponse::OpenMenu
            }
            MenuHandleGesture::Hovered => MenuHandleResponse::Nothing,
        }
    }

    pub fn remember_menu_handle(&mut self, output: &str, target: HoverTarget, anchor: Rectangle) {
        self.menu_handles
            .retain(|handle| handle.output != output || handle.target != target);
        self.menu_handles.push(PanelMenuHandle {
            output: output.to_owned(),
            target,
            anchor,
        });
    }

    fn menu_handle_anchors_on(&self, output: &str) -> Vec<Rectangle> {
        self.menu_handles
            .iter()
            .filter(|handle| handle.output == output)
            .map(|handle| handle.anchor)
            .collect()
    }

    #[must_use]
    pub fn is_held_open(&self) -> bool {
        self.transition.is_held_open()
    }

    fn occupy(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        anchor: Rectangle,
        announcement: HoverAnnouncement,
        retention: CardRetention,
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
                retention,
            },
            now,
        );
    }

    fn exit(&mut self, target: &HoverTarget, now: Instant) -> Option<Instant> {
        if self.transition.is_held_open() {
            return None;
        }
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
        if !self.transition.is_held_open() {
            self.hide_due = Some(due);
        }
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
        let catching_outputs = self
            .menu_handles
            .iter()
            .filter(|_| self.transition.is_held_open())
            .map(|handle| handle.output.clone());
        let wanted_outputs: BTreeSet<String> = self
            .transition
            .placement()
            .map(|placement| placement.output.clone())
            .into_iter()
            .chain(catching_outputs)
            .collect();
        let stale_surfaces: Vec<(String, Id)> = self
            .surfaces
            .iter()
            .filter(|(output, _)| !wanted_outputs.contains(*output))
            .map(|(output, surface)| (output.clone(), *surface))
            .collect();
        let closes: Vec<Task<SurfaceRequest>> = stale_surfaces
            .into_iter()
            .map(|(output, surface)| {
                self.surfaces.remove(&output);
                Task::done(SurfaceRequest::Close { surface })
            })
            .collect();
        let opens: Vec<Task<SurfaceRequest>> = wanted_outputs
            .iter()
            .map(|output| self.open_surface(output))
            .collect();
        let keyboard_grab = self.reconcile_keyboard_grab();
        Task::batch(closes.into_iter().chain(opens).chain([keyboard_grab]))
    }

    fn reconcile_keyboard_grab(&mut self) -> Task<SurfaceRequest> {
        let wanted_grab = self
            .transition
            .current()
            .filter(|_| self.transition.is_held_open())
            .and_then(|current| self.surface_on(&current.placement.output));
        if wanted_grab == self.keyboard_grab {
            return Task::none();
        }
        let released = self
            .keyboard_grab
            .filter(|surface| self.hosts(*surface))
            .map(|surface| {
                Task::done(SurfaceRequest::KeyboardInteractivity {
                    surface,
                    interactivity: KeyboardInteractivity::None,
                })
            });
        let grabbed = wanted_grab.map(|surface| {
            Task::done(SurfaceRequest::KeyboardInteractivity {
                surface,
                interactivity: KeyboardInteractivity::Exclusive,
            })
        });
        self.keyboard_grab = wanted_grab;
        Task::batch(released.into_iter().chain(grabbed))
    }

    pub fn forget(&mut self, surface: Id) {
        let Some(output) = self.output_of(surface) else {
            return;
        };
        self.forget_output(&output);
    }

    pub fn forget_output(&mut self, output: &str) {
        self.surfaces.remove(output);
        self.menu_handles.retain(|handle| handle.output != output);
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

    fn outside_press_on<Message: From<PopupMessage>>(&self, surface: Id) -> OutsidePress<Message> {
        if !self.transition.is_held_open() {
            return OutsidePress::PassesThrough;
        }
        OutsidePress::Dismisses {
            dismissal: Message::from(PopupMessage::HoverCard(HoverCardMessage::DismissHeldOpen)),
            reachable_below: self
                .output_of(surface)
                .map_or_else(Vec::new, |output| self.menu_handle_anchors_on(&output)),
        }
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
                self.outside_press_on(surface),
                config,
                PopupAppearances {
                    overlay: surface_style.overlay_appearance,
                    menu: surface_style.menu_appearance,
                },
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
                    progress: self.transition.morph_progress(
                        clock.instant,
                        config.overlay_surfaces.surface_morph_duration,
                    ),
                    shown: self.transition.shown_target(),
                },
                self.outside_press_on(surface),
                config,
                PopupAppearances {
                    overlay: surface_style.overlay_appearance,
                    menu: surface_style.menu_appearance,
                },
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
    pub menu_appearance: LayerAppearance,
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

#[cfg(test)]
mod tests {
    use iced::{
        Point, Rectangle, Size,
        time::{Duration, Instant},
    };
    use sottomano_config::screen_edges::Position;

    use super::{HIDE_GRACE, HoverCardHost, MenuHandleResponse};
    use crate::{
        clock::Clock,
        hover_cards::{HoverAnnouncement, HoverCardMessage, HoverTarget, MenuHandleGesture},
        layer_surfaces::PanelEdge,
    };

    fn top_edge() -> PanelEdge {
        PanelEdge {
            output: "DP-1".to_owned(),
            position: Position::Top,
        }
    }

    fn workspace_card() -> HoverTarget {
        HoverTarget::Workspace(7)
    }

    fn leave_every_way(host: &mut HoverCardHost, now: Instant) -> Instant {
        let _exited = host.update(
            HoverCardMessage::Exit {
                target: workspace_card(),
            },
            now,
        );
        let _left_card = host.update(HoverCardMessage::ExitHoverCard, now);
        let due = host.leave_after_grace(now);
        host.leave_elapsed(due + HIDE_GRACE, None);
        due
    }

    #[test]
    fn a_held_open_card_outlasts_the_pointer_leaving() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        host.open_until_dismissed(
            &top_edge(),
            workspace_card(),
            Rectangle::default(),
            HoverAnnouncement::default(),
            now,
        );
        let _due = leave_every_way(&mut host, now);
        assert_eq!(host.target(), Some(&workspace_card()));
        assert!(host.is_held_open());
        assert_eq!(host.hide_due(), None);
    }

    #[test]
    fn a_hovered_card_closes_when_the_pointer_leaves() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        host.open(
            &top_edge(),
            workspace_card(),
            Rectangle::default(),
            HoverAnnouncement::default(),
            now,
        );
        assert!(!host.is_held_open());
        let _due = leave_every_way(&mut host, now);
        assert_eq!(host.target(), None);
    }

    #[test]
    fn a_handle_laid_out_again_replaces_its_anchor_on_that_output_only() {
        let first = Rectangle::new(Point::new(1.0, 0.0), Size::new(10.0, 10.0));
        let moved = Rectangle::new(Point::new(2.0, 0.0), Size::new(10.0, 10.0));
        let mut host = HoverCardHost::default();
        host.remember_menu_handle("DP-1", workspace_card(), first);
        host.remember_menu_handle("DP-2", workspace_card(), first);
        host.remember_menu_handle("DP-1", workspace_card(), moved);
        assert_eq!(host.menu_handle_anchors_on("DP-1"), [moved]);
        assert_eq!(host.menu_handle_anchors_on("DP-2"), [first]);
        host.forget_output("DP-2");
        assert!(host.menu_handle_anchors_on("DP-2").is_empty());
    }

    fn other_menu_handle() -> HoverTarget {
        HoverTarget::Workspace(8)
    }

    fn host_holding_a_menu_open() -> HoverCardHost {
        let mut host = HoverCardHost::default();
        host.open_until_dismissed(
            &top_edge(),
            workspace_card(),
            Rectangle::default(),
            HoverAnnouncement::default(),
            Clock::real().instant,
        );
        host
    }

    #[test]
    fn with_no_menu_open_only_a_press_opens_one() {
        let host = HoverCardHost::default();
        assert_eq!(
            host.response_to_menu_handle(MenuHandleGesture::Hovered, &workspace_card(), "DP-1"),
            MenuHandleResponse::Nothing
        );
        assert_eq!(
            host.response_to_menu_handle(MenuHandleGesture::Pressed, &workspace_card(), "DP-1"),
            MenuHandleResponse::OpenMenu
        );
    }

    #[test]
    fn an_open_menu_switches_on_the_first_hover_or_press_of_another_handle() {
        let host = host_holding_a_menu_open();
        for gesture in [MenuHandleGesture::Hovered, MenuHandleGesture::Pressed] {
            assert_eq!(
                host.response_to_menu_handle(gesture, &other_menu_handle(), "DP-1"),
                MenuHandleResponse::OpenMenu
            );
        }
    }

    #[test]
    fn the_handle_of_the_open_menu_closes_it_on_press_and_ignores_hover() {
        let host = host_holding_a_menu_open();
        assert_eq!(
            host.response_to_menu_handle(MenuHandleGesture::Pressed, &workspace_card(), "DP-1"),
            MenuHandleResponse::CloseMenu
        );
        assert_eq!(
            host.response_to_menu_handle(MenuHandleGesture::Hovered, &workspace_card(), "DP-1"),
            MenuHandleResponse::Nothing
        );
    }

    #[test]
    fn hovering_a_handle_on_another_output_leaves_the_open_menu() {
        let host = host_holding_a_menu_open();
        assert_eq!(
            host.response_to_menu_handle(MenuHandleGesture::Hovered, &other_menu_handle(), "DP-2"),
            MenuHandleResponse::Nothing
        );
    }

    #[test]
    fn a_held_open_menu_catches_presses_on_every_output_until_dismissed() {
        let now = Clock::real().instant;
        let fade = Duration::from_millis(1);
        let anchor = Rectangle::default();
        let mut host = host_holding_a_menu_open();
        host.remember_menu_handle("DP-1", workspace_card(), anchor);
        host.remember_menu_handle("DP-2", workspace_card(), anchor);
        let _opened = host.reconcile_surfaces(now, fade);
        assert!(host.surface_on("DP-1").is_some());
        assert!(host.surface_on("DP-2").is_some());
        host.dismiss(None, now);
        let _closed = host.reconcile_surfaces(now, fade);
        assert!(host.surface_on("DP-1").is_none());
        assert!(host.surface_on("DP-2").is_none());
    }

    #[test]
    fn a_hovered_card_stays_on_its_own_output() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        host.remember_menu_handle("DP-2", other_menu_handle(), Rectangle::default());
        host.open(
            &top_edge(),
            workspace_card(),
            Rectangle::default(),
            HoverAnnouncement::default(),
            now,
        );
        let _opened = host.reconcile_surfaces(now, Duration::from_millis(1));
        assert!(host.surface_on("DP-1").is_some());
        assert!(host.surface_on("DP-2").is_none());
    }

    #[test]
    fn dismissing_a_held_open_card_closes_it() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        host.open_until_dismissed(
            &top_edge(),
            workspace_card(),
            Rectangle::default(),
            HoverAnnouncement::default(),
            now,
        );
        host.dismiss(None, now);
        assert_eq!(host.target(), None);
        assert!(!host.is_held_open());
    }
}
