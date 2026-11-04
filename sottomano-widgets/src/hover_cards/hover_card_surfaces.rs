use std::collections::BTreeMap;

use iced::{
    Element, Rectangle, Task, task,
    time::{Duration, Instant},
    widget::{container, mouse_area},
    window::Id,
};
use iced_exwlshell::reexport::KeyboardInteractivity;
use sottomano_config::{Config, screen_edges::Position};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    group_menus::cascading_layout::cascade_row::{CascadeOrientation, cascade_row},
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, PopupMessage, WidgetAnchor,
        anchored_popup::{
            OutsidePress, PopupAppearances, PopupFraming, PopupMorph, Rendering, anchored_popup,
        },
        hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_content::HoverCardContent,
        hover_card_transition::{
            CardRetention, Departed, DepartedContent, HoverCardTransition, Occupant, Placement,
            ShownTarget,
        },
        laid_out_anchors::LaidOutAnchors,
    },
    layer_surfaces::{PanelEdge, SurfaceRequest},
    widgets::{shell_layer_appearance::LayerAppearance, window_icon::WindowAssets},
};

pub const HIDE_GRACE: Duration = Duration::from_millis(250);
const SWITCH_INTENT: Duration = Duration::from_millis(180);

pub struct PendingSwitch {
    pub parent: PanelEdge,
    pub target: HoverTarget,
    pub announcement: HoverAnnouncement,
    _intent_timer: task::Handle,
}

#[derive(Default)]
pub struct HoverCardHost {
    transition: HoverCardTransition,
    pending_switch: Option<PendingSwitch>,
    hide_due: Option<Instant>,
    pointer_over_hover_card: bool,
    keyboard_grab: Option<Id>,
    laid_out_anchors: LaidOutAnchors,
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
            | HoverCardMessage::AnchorLaidOut(..)
            | HoverCardMessage::LaidOut { .. }
            | HoverCardMessage::DismissHeldOpen
            | HoverCardMessage::MenuHandlePressed { .. }
            | HoverCardMessage::MenuHandlePressedAnchor { .. }
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
        announcement: HoverAnnouncement,
    ) {
        self.occupy(
            parent,
            target,
            announcement,
            CardRetention::UntilPointerLeaves,
        );
    }

    pub fn open_until_dismissed(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        announcement: HoverAnnouncement,
    ) {
        self.occupy(parent, target, announcement, CardRetention::UntilDismissed);
    }

    pub fn remember_anchor(
        &mut self,
        output: &str,
        widget_anchor: WidgetAnchor,
        now: Instant,
    ) -> Option<Instant> {
        let hid_the_shown_card = !widget_anchor.visible
            && self
                .transition
                .shown_target_on_output()
                .is_some_and(|(target, shown_output)| {
                    shown_output == output && *target == widget_anchor.target
                });
        self.laid_out_anchors.remember(output, widget_anchor);
        hid_the_shown_card.then(|| self.leave_after_grace(now))
    }

    #[must_use]
    pub fn shown_anchor(&self) -> Option<Rectangle> {
        let (target, output) = self.transition.shown_target_on_output()?;
        self.laid_out_anchors.anchor_of(output, target)
    }

    #[must_use]
    pub fn is_held_open(&self) -> bool {
        self.transition.is_held_open()
    }

    fn occupy(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        announcement: HoverAnnouncement,
        retention: CardRetention,
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
        self.transition.show(Occupant {
            target,
            announcement,
            placement: Placement {
                position: parent.position,
                output: parent.output.clone(),
            },
            retention,
        });
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

    #[must_use]
    pub fn placement(&self) -> Option<&Placement> {
        self.transition.placement()
    }

    #[must_use]
    pub fn is_hiding(&self) -> bool {
        self.transition.hiding_since().is_some()
    }

    pub fn reconcile_with_panels(
        &mut self,
        panel_surfaces: &BTreeMap<String, Id>,
        now: Instant,
        fade: Duration,
    ) -> Task<SurfaceRequest> {
        let finished_hiding = self
            .transition
            .hiding_since()
            .is_some_and(|since| now >= since + fade);
        let lost_its_panel = self
            .transition
            .placement()
            .is_some_and(|placement| !panel_surfaces.contains_key(&placement.output));
        if finished_hiding || lost_its_panel {
            self.transition.reset();
        }
        self.reconcile_keyboard_grab(panel_surfaces)
    }

    fn reconcile_keyboard_grab(
        &mut self,
        panel_surfaces: &BTreeMap<String, Id>,
    ) -> Task<SurfaceRequest> {
        let wanted_grab = self
            .transition
            .current()
            .filter(|_| self.transition.is_held_open())
            .and_then(|current| panel_surfaces.get(&current.placement.output).copied());
        if wanted_grab == self.keyboard_grab {
            return Task::none();
        }
        let released = self
            .keyboard_grab
            .filter(|surface| panel_surfaces.values().any(|panel| panel == surface))
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

    pub fn forget_output(&mut self, output: &str) {
        self.laid_out_anchors.forget_output(output);
        if self
            .transition
            .placement()
            .is_some_and(|placement| placement.output == output)
        {
            self.transition.reset();
        }
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

    fn outside_press_on<Message: From<PopupMessage>>(&self, output: &str) -> OutsidePress<Message> {
        if !self.transition.is_held_open() {
            return OutsidePress::PassesThrough;
        }
        OutsidePress::Dismisses {
            dismissal: Message::from(PopupMessage::HoverCard(HoverCardMessage::DismissHeldOpen)),
            reachable_below: self.laid_out_anchors.menu_handles_on(output),
        }
    }

    pub fn view<'a, Message: From<PopupMessage> + Clone + 'a>(
        &self,
        output: &str,
        config: &'a Config,
        surface_style: HoverCardSurfaceStyle,
        assets: WindowAssets<'a>,
        content_of: impl Fn(&HoverTarget) -> HoverContent<'a, Message>,
        clock: Clock,
    ) -> Element<'a, Message> {
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
        let Some((placement, anchor)) = self
            .transition
            .placement()
            .filter(|placement| placement.output == output)
            .zip(self.shown_anchor())
        else {
            return anchored_popup(
                Vec::new(),
                Rectangle::default(),
                Position::Top,
                PopupMorph {
                    duration: config.overlay_surfaces.surface_morph_duration,
                    shown: ShownTarget::Nothing,
                },
                self.outside_press_on(output),
                config,
                PopupAppearances {
                    overlay: surface_style.overlay_appearance,
                    menu: surface_style.menu_appearance,
                },
            );
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
        container(anchored_popup(
            renderings,
            anchor,
            placement.position,
            PopupMorph {
                duration: config.overlay_surfaces.surface_morph_duration,
                shown: self.transition.shown_target(),
            },
            self.outside_press_on(output),
            config,
            PopupAppearances {
                overlay: surface_style.overlay_appearance,
                menu: surface_style.menu_appearance,
            },
        ))
        .padding(surface_style.float_offset_px)
        .into()
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iced::{
        Point, Rectangle, Size,
        time::{Duration, Instant},
        window::Id,
    };
    use sottomano_config::screen_edges::Position;

    use super::{HIDE_GRACE, HoverCardHost};
    use crate::{
        clock::Clock,
        hover_cards::{
            CardOpening, HoverAnnouncement, HoverCardMessage, HoverTarget, WidgetAnchor,
        },
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
        host.open_until_dismissed(&top_edge(), workspace_card(), HoverAnnouncement::default());
        let _due = leave_every_way(&mut host, now);
        assert_eq!(host.target(), Some(&workspace_card()));
        assert!(host.is_held_open());
        assert_eq!(host.hide_due(), None);
    }

    #[test]
    fn a_hovered_card_closes_when_the_pointer_leaves() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        host.open(&top_edge(), workspace_card(), HoverAnnouncement::default());
        assert!(!host.is_held_open());
        let _due = leave_every_way(&mut host, now);
        assert_eq!(host.target(), None);
    }

    fn anchor_at(horizontal_position: f32) -> Rectangle {
        Rectangle::new(Point::new(horizontal_position, 0.0), Size::new(10.0, 10.0))
    }

    fn widget_at(horizontal_position: f32, opening: CardOpening) -> WidgetAnchor {
        WidgetAnchor {
            target: workspace_card(),
            opening,
            anchor: anchor_at(horizontal_position),
            visible: true,
        }
    }

    fn remember(host: &mut HoverCardHost, output: &str, widget_anchor: WidgetAnchor) {
        let _hide_due = host.remember_anchor(output, widget_anchor, Clock::real().instant);
    }

    #[test]
    fn an_open_card_reads_the_newest_anchor_of_its_output() {
        let mut host = HoverCardHost::default();
        let opening = CardOpening::OnHover;
        remember(&mut host, "DP-1", widget_at(1.0, opening));
        remember(&mut host, "DP-2", widget_at(50.0, opening));
        host.open(&top_edge(), workspace_card(), HoverAnnouncement::default());
        assert_eq!(host.shown_anchor(), Some(anchor_at(1.0)));
        remember(&mut host, "DP-1", widget_at(2.0, opening));
        assert_eq!(host.shown_anchor(), Some(anchor_at(2.0)));
    }

    #[test]
    fn a_held_open_menu_reads_the_newest_anchor_of_its_handle() {
        let mut host = host_holding_a_menu_open();
        let opening = CardOpening::OnPress;
        remember(&mut host, "DP-1", widget_at(1.0, opening));
        remember(&mut host, "DP-1", widget_at(2.0, opening));
        assert_eq!(host.shown_anchor(), Some(anchor_at(2.0)));
    }

    #[test]
    fn a_card_without_a_reported_anchor_has_no_position() {
        let host = host_holding_a_menu_open();
        assert_eq!(host.shown_anchor(), None);
    }

    #[test]
    fn a_shown_card_leaves_when_its_widget_is_hidden_even_with_the_pointer_on_it() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        remember(&mut host, "DP-1", widget_at(1.0, CardOpening::OnHover));
        host.open(&top_edge(), workspace_card(), HoverAnnouncement::default());
        let _on_card = host.update(HoverCardMessage::EnterHoverCard, now);
        let due = host
            .remember_anchor(
                "DP-1",
                WidgetAnchor {
                    visible: false,
                    ..widget_at(1.0, CardOpening::OnHover)
                },
                now,
            )
            .expect("the hidden widget's card leaves");
        host.leave_elapsed(due, None);
        assert_eq!(host.target(), None);
    }

    #[test]
    fn a_hidden_widget_on_another_output_leaves_the_card_alone() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        remember(&mut host, "DP-1", widget_at(1.0, CardOpening::OnHover));
        host.open(&top_edge(), workspace_card(), HoverAnnouncement::default());
        let hide_due = host.remember_anchor(
            "DP-2",
            WidgetAnchor {
                visible: false,
                ..widget_at(1.0, CardOpening::OnHover)
            },
            now,
        );
        assert_eq!(hide_due, None);
    }

    #[test]
    fn a_held_open_menu_stays_when_its_handle_is_hidden() {
        let now = Clock::real().instant;
        let mut host = host_holding_a_menu_open();
        let due = host.remember_anchor(
            "DP-1",
            WidgetAnchor {
                visible: false,
                ..widget_at(1.0, CardOpening::OnPress)
            },
            now,
        );
        host.leave_elapsed(due.map_or(now, |due| due + HIDE_GRACE), None);
        assert_eq!(host.target(), Some(&workspace_card()));
    }

    #[test]
    fn a_disconnect_removes_anchors_and_the_shown_card() {
        let mut host = HoverCardHost::default();
        remember(&mut host, "DP-1", widget_at(1.0, CardOpening::OnHover));
        host.open(&top_edge(), workspace_card(), HoverAnnouncement::default());
        host.forget_output("DP-1");
        assert_eq!(host.target(), None);
        host.open(&top_edge(), workspace_card(), HoverAnnouncement::default());
        assert_eq!(host.shown_anchor(), None);
    }

    fn host_holding_a_menu_open() -> HoverCardHost {
        let mut host = HoverCardHost::default();
        host.open_until_dismissed(&top_edge(), workspace_card(), HoverAnnouncement::default());
        host
    }

    fn panels_on(outputs: &[&str]) -> BTreeMap<String, Id> {
        outputs
            .iter()
            .map(|output| ((*output).to_owned(), Id::unique()))
            .collect()
    }

    #[test]
    fn a_card_closes_when_its_output_loses_the_panel() {
        let now = Clock::real().instant;
        let fade = Duration::from_millis(1);
        let mut host = host_holding_a_menu_open();
        let _kept = host.reconcile_with_panels(&panels_on(&["DP-1", "DP-2"]), now, fade);
        assert_eq!(host.target(), Some(&workspace_card()));
        let _lost = host.reconcile_with_panels(&panels_on(&["DP-2"]), now, fade);
        assert_eq!(host.target(), None);
    }

    #[test]
    fn a_held_open_menu_grabs_the_keyboard_on_its_panel_surface() {
        let now = Clock::real().instant;
        let fade = Duration::from_millis(1);
        let panels = panels_on(&["DP-1"]);
        let mut host = host_holding_a_menu_open();
        let _grabbed = host.reconcile_with_panels(&panels, now, fade);
        assert_eq!(host.keyboard_grab, panels.get("DP-1").copied());
        host.dismiss(None, now);
        let _released = host.reconcile_with_panels(&panels, now, fade);
        assert_eq!(host.keyboard_grab, None);
    }

    #[test]
    fn dismissing_a_held_open_card_closes_it() {
        let now = Clock::real().instant;
        let mut host = HoverCardHost::default();
        host.open_until_dismissed(&top_edge(), workspace_card(), HoverAnnouncement::default());
        host.dismiss(None, now);
        assert_eq!(host.target(), None);
        assert!(!host.is_held_open());
    }
}
