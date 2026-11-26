mod group_expansion_state;
mod group_hover_card;
mod group_menu_sections;
pub(crate) mod group_panel_view;
#[cfg(test)]
mod indicator_group_content_tests;
mod indicator_listing;

use std::{collections::BTreeSet, rc::Rc};

use chrono::Locale;
use sottomano_toml_config::severity::Severity;

pub use self::{
    group_expansion_state::{
        ExpansionChange, IndicatorGroupExpansion, IndicatorGroupExpansionMessage, KeyedExpansions,
    },
    indicator_listing::IndicatorListing,
};
use crate::{
    clock::Clock,
    group_indicators::{aligned_quantity::IndicatorAlignment, group_indicator::GroupIndicator},
    group_menus::group_menu_state::{
        AlertPromotion, GroupMenuPin, GroupMenuTopic, PanelPin, PanelPinDuration, PanelRaise,
    },
    hover_cards::{
        HoverCardMessage, IndicatorKey, PanelPinOrigin, PopupMessage,
        hover_card_body::hover_card_source_delivery::SectionSource,
    },
    indicator_groups::{
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::{announced_level::LevelToastRequest, ongoing_toast::OngoingToast},
};

#[derive(Debug, Clone)]
pub enum GroupEvent {
    Interaction(IndicatorGroupInteraction),
    Popup(PopupMessage),
}

impl From<HoverCardMessage> for GroupEvent {
    fn from(hover_card: HoverCardMessage) -> Self {
        GroupEvent::Popup(PopupMessage::HoverCard(hover_card))
    }
}

impl From<PopupMessage> for GroupEvent {
    fn from(popup: PopupMessage) -> Self {
        GroupEvent::Popup(popup)
    }
}

impl<Interaction: Into<IndicatorGroupInteraction>> From<Interaction> for GroupEvent {
    fn from(interaction: Interaction) -> Self {
        GroupEvent::Interaction(interaction.into())
    }
}

#[derive(Default)]
pub struct GroupToastRequests {
    pub ongoing_toasts: Vec<OngoingToast<IndicatorGroupInteraction>>,
    pub levels: Vec<LevelToastRequest<IndicatorGroupInteraction>>,
}

impl FromIterator<GroupToastRequests> for GroupToastRequests {
    fn from_iter<I: IntoIterator<Item = GroupToastRequests>>(groups: I) -> Self {
        groups.into_iter().fold(
            GroupToastRequests {
                ongoing_toasts: Vec::new(),
                levels: Vec::new(),
            },
            |mut requests, group| {
                requests.ongoing_toasts.extend(group.ongoing_toasts);
                requests.levels.extend(group.levels);
                requests
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardTitleClash {
    pub indicator_name: String,
    pub clashing_title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleScrollForwarding {
    NotForwarded,
    Forwarded(&'static str),
}

pub struct IndicatorGroupContent<'view> {
    pub(crate) group_identity: IndicatorGroupIdentity,
    pub(crate) group_title: IndicatorGroupTitle,
    expansion: &'view IndicatorGroupExpansion,
    on_expansion: Rc<dyn Fn(IndicatorGroupExpansionMessage) -> IndicatorGroupInteraction + 'view>,
    headline_indicator: GroupIndicator<'view>,
    scroll_forwarding: HandleScrollForwarding,
    pub(crate) listing: IndicatorListing<'view>,
    panel_pin: PanelPinOrigin,
}

impl<'view> IndicatorGroupContent<'view> {
    pub fn expandable<Interaction: Into<IndicatorGroupInteraction>>(
        family: IndicatorGroupFamily,
        headline_indicator: GroupIndicator<'view>,
        expansion: &'view IndicatorGroupExpansion,
        on_expansion: impl Fn(IndicatorGroupExpansionMessage) -> Interaction + 'view,
        listing: IndicatorListing<'view>,
    ) -> Self {
        Self::expandable_instance(
            IndicatorGroupIdentity::sole(family),
            IndicatorGroupTitle::fixed(family.title()),
            headline_indicator,
            expansion,
            on_expansion,
            listing,
        )
    }

    pub fn expandable_instance<Interaction: Into<IndicatorGroupInteraction>>(
        group_identity: IndicatorGroupIdentity,
        group_title: IndicatorGroupTitle,
        headline_indicator: GroupIndicator<'view>,
        expansion: &'view IndicatorGroupExpansion,
        on_expansion: impl Fn(IndicatorGroupExpansionMessage) -> Interaction + 'view,
        listing: IndicatorListing<'view>,
    ) -> Self {
        Self {
            group_identity,
            group_title,
            expansion,
            on_expansion: Rc::new(move |message| on_expansion(message).into()),
            headline_indicator,
            scroll_forwarding: HandleScrollForwarding::NotForwarded,
            listing,
            panel_pin: PanelPinOrigin::Unpinned,
        }
    }

    #[must_use]
    pub fn card_title_clashes(&self, clock: Clock) -> Vec<CardTitleClash> {
        let group_title = self.group_title.to_string();
        self.listing
            .iter()
            .flat_map(|indicator| {
                let indicator_name = indicator.indicator_name().into_owned();
                let group_title = group_title.clone();
                (indicator.card)(clock)
                    .into_iter()
                    .map(|section| {
                        let source_name = match section.source {
                            SectionSource::Snapshot { source_name, .. } => source_name.to_owned(),
                            SectionSource::Local => String::new(),
                        };
                        (section.title.text().to_owned(), source_name)
                    })
                    .filter(|(section_title, source_name)| {
                        section_title.is_empty()
                            || *section_title == indicator_name
                            || *section_title == group_title
                            || section_title == source_name
                    })
                    .map(|(section_title, _)| CardTitleClash {
                        indicator_name: indicator_name.clone(),
                        clashing_title: section_title,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[must_use]
    pub fn clashing_indicator_names(&self) -> Vec<String> {
        let group_title = self.group_title.to_string();
        let mut seen_names = BTreeSet::new();
        self.listing
            .iter()
            .filter(|indicator| indicator.menu_pin(&self.group_identity).is_some())
            .map(|indicator| indicator.indicator_name().into_owned())
            .filter(|indicator_name| {
                *indicator_name == group_title || !seen_names.insert(indicator_name.clone())
            })
            .collect()
    }

    pub(crate) fn panel_raises(
        &self,
        menu: GroupMenuTopic,
        promotion_severity: Severity,
    ) -> Vec<PanelRaise> {
        self.listing
            .iter()
            .filter(|indicator| {
                indicator.alert_promotion(promotion_severity) == AlertPromotion::Promoted
            })
            .filter_map(|indicator| indicator.menu_pin(&self.group_identity))
            .map(|entry| PanelRaise { menu, entry })
            .collect()
    }

    #[must_use]
    pub fn pinned_from(self, menu: GroupMenuTopic) -> Self {
        Self {
            panel_pin: PanelPinOrigin::Pinned(Box::new(PanelPin {
                menu,
                entry: GroupMenuPin::Group(self.group_identity.clone()),
                duration: PanelPinDuration::Permanent,
            })),
            ..self
        }
    }

    #[must_use]
    pub fn forwards_scroll(self, scroll_does: &'static str) -> Self {
        Self {
            scroll_forwarding: HandleScrollForwarding::Forwarded(scroll_does),
            ..self
        }
    }

    #[must_use]
    pub fn only_aligned_indicators(self) -> Option<Self> {
        self.retaining_indicators(|alignment| alignment != IndicatorAlignment::Unaligned)
    }

    #[must_use]
    pub fn only_unaligned_indicators(self) -> Option<Self> {
        self.retaining_indicators(|alignment| alignment == IndicatorAlignment::Unaligned)
    }

    fn retaining_indicators(
        self,
        keeps_alignment: impl Fn(IndicatorAlignment) -> bool,
    ) -> Option<Self> {
        let listing = self.listing.retaining_listed(keeps_alignment);
        if let IndicatorListing::Listed(listed_indicators) = &listing
            && listed_indicators.is_empty()
        {
            return None;
        }
        Some(Self { listing, ..self })
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests {
        let expand = (self.on_expansion)(IndicatorGroupExpansionMessage::Expand);
        self.listing
            .iter()
            .map(|indicator| {
                indicator.toast_requests(&self.group_identity, expand.clone(), locale, clock)
            })
            .collect()
    }

    pub(crate) fn highest_alert_severity(&self) -> Severity {
        self.listing
            .iter()
            .filter_map(GroupIndicator::shown_alert_severity)
            .max()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn indicator(&self, indicator_key: &IndicatorKey) -> Option<&GroupIndicator<'view>> {
        if indicator_key.group != self.group_identity {
            return None;
        }
        self.listing
            .iter()
            .find(|indicator| indicator.indicator_name() == indicator_key.name)
    }

    #[must_use]
    pub fn indicator_keys(&self) -> Vec<IndicatorKey> {
        self.listing
            .iter()
            .map(|indicator| indicator.indicator_key(&self.group_identity))
            .collect()
    }
}
