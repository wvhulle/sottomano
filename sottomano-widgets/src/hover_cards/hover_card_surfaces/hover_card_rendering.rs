use corredo::popovers::{
    CascadeOrientation, OutsidePress, PopupFraming, PopupRendering, cascade_row,
    dropped_anchor_watch,
};
use iced::{
    Element, Rectangle,
    widget::{container, mouse_area, stack},
};
use sottomano_toml_config::{Config, screen_edges::Position};

use super::HoverCardHost;
use crate::{
    clock::Clock,
    hover_cards::{
        HoverCardMessage, HoverTarget, PopupMessage,
        anchored_popup::{PopupAppearances, PopupMorph, PressCatcher, anchored_popup},
        hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_content::HoverCardContent,
        hover_card_transition::{CardRetention, Departed, DepartedContent, ShownTarget},
    },
    widgets::window_icon::WindowAssets,
};

impl HoverCardHost {
    #[must_use]
    pub fn departure<'a, Message>(
        &self,
        content_of: impl FnOnce(&HoverTarget) -> Option<HoverContent<'a, Message>>,
    ) -> Option<Departed> {
        let current = self.transition.current()?;
        let departed_content = match content_of(&current.target)? {
            HoverContent::Described(hover_card) => DepartedContent::Described(hover_card),
            HoverContent::Interactive { .. } | HoverContent::Cascading { .. } => {
                DepartedContent::Interactive
            }
        };
        Some(Departed {
            target: current.target.clone(),
            content: departed_content,
            placement: current.placement.clone(),
        })
    }

    fn press_catcher_on<Message: From<PopupMessage>>(&self, output: &str) -> PressCatcher<Message> {
        if self.transition.shown_retention() != CardRetention::UntilDismissed {
            return PressCatcher::without_holes(OutsidePress::PassesThrough);
        }
        PressCatcher {
            outside_press: OutsidePress::Dismisses {
                dismissal: Message::from(PopupMessage::HoverCard(
                    HoverCardMessage::DismissHeldOpen,
                )),
                reachable_below: self.laid_out_anchors.menu_handles_on(output),
            },
            catch_area_holes: self
                .toast_cards_by_output
                .get(output)
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub fn view<'a, Message: From<PopupMessage> + Clone + 'a>(
        &self,
        output: &str,
        config: &'a Config,
        surface_style: HoverCardSurfaceStyle,
        assets: WindowAssets<'a>,
        content_of: impl Fn(&HoverTarget) -> Option<HoverContent<'a, Message>>,
        clock: Clock,
    ) -> Element<'a, Message> {
        let describe = move |hover_card: &HoverCard| -> Element<'a, Message> {
            Element::from(HoverCardContent::new(hover_card, config, assets, clock).view())
                .map(Message::from)
        };
        let render = |content: HoverContent<'a, Message>| match content {
            HoverContent::Described(hover_card) => Rendered::framed(describe(&hover_card)),
            HoverContent::Interactive { panel } => Rendered::framed(panel),
            HoverContent::Cascading {
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
                self.press_catcher_on(output),
                config,
                surface_style.popup,
            );
        };

        let mut renderings = Vec::new();
        if let Some(previous) = self.transition.previous() {
            let departed = match &previous.content {
                DepartedContent::Described(hover_card) => {
                    Some(HoverContent::Described(hover_card.clone()))
                }
                DepartedContent::Interactive => content_of(&previous.target),
            };
            if let Some(departed) = departed {
                let Rendered { element, framing } = render(departed);
                renderings.push(PopupRendering::outgoing(element).framing(framing));
            }
        }
        if let Some(current) = self.transition.current()
            && let Some(content) = content_of(&current.target)
        {
            let Rendered { element, framing } = render(content);
            renderings.push(PopupRendering::incoming(hoverable_content(element)).framing(framing));
        }
        let shown_presence = self.transition.current().and_then(|current| {
            self.laid_out_anchors
                .presence_of(&current.placement.output, &current.target)
        });
        let card = container(anchored_popup(
            renderings,
            anchor,
            placement.position,
            PopupMorph {
                duration: config.overlay_surfaces.surface_morph_duration,
                shown: self.transition.shown_target(),
            },
            self.press_catcher_on(output),
            config,
            surface_style.popup,
        ))
        .padding(surface_style.float_offset_px);
        let Some(shown_presence) = shown_presence else {
            return card.into();
        };
        stack![
            card,
            dropped_anchor_watch(
                shown_presence,
                Message::from(PopupMessage::HoverCard(
                    HoverCardMessage::ShownWidgetDropped
                )),
            ),
        ]
        .into()
    }
}

pub enum HoverContent<'a, Message> {
    Described(HoverCard),
    Interactive {
        panel: Element<'a, Message>,
    },
    Cascading {
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

#[derive(Debug, Clone, Copy)]
pub struct HoverCardSurfaceStyle {
    pub popup: PopupAppearances,
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
