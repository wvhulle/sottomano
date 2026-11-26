use super::{HandleScrollForwarding, IndicatorGroupContent, IndicatorListing};
use crate::{
    clock::Clock,
    group_indicators::group_indicator::{GroupIndicator, RowHoverCard},
    hover_cards::{CardHeader, HoverTarget, MouseGesture, PressAction, hover_card_body::Section},
};

impl<'view> IndicatorGroupContent<'view> {
    #[must_use]
    pub fn hovered_indicator(&self, target: &HoverTarget) -> Option<&GroupIndicator<'view>> {
        match target.headline_group() {
            Some(group_identity) if *group_identity == self.group_identity => {
                Some(&self.headline_indicator)
            }
            Some(_) => None,
            None => target
                .indicator_key()
                .and_then(|indicator_key| self.indicator(indicator_key)),
        }
    }

    fn handle_actions(&self) -> Vec<PressAction> {
        let face_actions = self.headline_indicator.press_actions();
        let expansion_action = match &self.listing {
            IndicatorListing::Listed(listed_indicators) if listed_indicators.is_empty() => None,
            IndicatorListing::Listed(_) | IndicatorListing::Incomplete { .. } => {
                Some(PressAction::group_handle(self.expansion.state))
            }
        };
        let handle_actions: Vec<PressAction> = expansion_action
            .into_iter()
            .chain(match self.scroll_forwarding {
                HandleScrollForwarding::Forwarded(scroll_does) => {
                    Some(PressAction::new(MouseGesture::Scroll, scroll_does))
                }
                HandleScrollForwarding::NotForwarded => None,
            })
            .filter(|handle_action| {
                face_actions
                    .iter()
                    .all(|face_action| face_action.gesture != handle_action.gesture)
            })
            .collect();
        face_actions.into_iter().chain(handle_actions).collect()
    }

    fn face_card(&self, clock: Clock) -> Vec<Section> {
        (self.headline_indicator.card)(clock)
            .into_iter()
            .chain(Section::press_actions(self.handle_actions()))
            .collect()
    }

    #[must_use]
    pub fn hover_card(&self, target: &HoverTarget, clock: Clock) -> Option<RowHoverCard> {
        let sections = match target.headline_group() {
            Some(group_identity) if *group_identity == self.group_identity => self.face_card(clock),
            Some(_) => return None,
            None => self
                .indicator(target.indicator_key()?)?
                .card_with_actions(clock),
        };
        let header = match target {
            HoverTarget::PinnedOnPanel(panel_pin) => CardHeader::Pinned {
                group_title: self.group_title.clone(),
                pin: panel_pin.clone(),
            },
            HoverTarget::Indicator(_)
            | HoverTarget::GroupHeadline(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::WindowTopMenu(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => CardHeader::Unheaded,
        };
        Some(RowHoverCard { sections, header })
    }
}
