use std::collections::BTreeMap;

use sottomano_desktop_sources::network_connectivity::tailscale_status::{
    NodeCapabilities, Tailnet, Tailscale,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const NO_TAGS_TEXT: &str = "none";
const BARE_WORD_TEXT: &str = "enabled";
const UNGROUPED_FEATURES_LABEL: &str = "Features";
const SHARED_WORD_MINIMUM: usize = 2;

#[derive(Clone, Copy)]
pub(super) struct OwnNodeIndicator<'view> {
    tailnet: &'view Tailnet,
}

impl<'view> OwnNodeIndicator<'view> {
    pub(super) fn running_own_node(tailscale: &'view Tailscale) -> Option<Self> {
        match tailscale {
            Tailscale::Running(tailnet) => Some(Self { tailnet }),
            Tailscale::Stopped | Tailscale::Starting => None,
        }
    }

    fn own_tags_text(self) -> String {
        match self.tailnet.own_node.tags.as_slice() {
            [] => NO_TAGS_TEXT.to_owned(),
            tags => tags.join(", "),
        }
    }

    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            "This machine",
            Phosphor::IdentificationBadge(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.own_tags_text()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(self) -> Vec<Section> {
        vec![
            Section::new(
                "Identity",
                dictionary_table([
                    LabeledValue::new("Machine", self.tailnet.own_node.name.as_str()),
                    LabeledValue::new("Tags", self.own_tags_text()),
                ]),
            ),
            Section::new(
                "Capabilities",
                dictionary_table(capability_rows(&self.tailnet.own_capabilities)),
            ),
        ]
    }
}

fn capitalized_word(word: &str) -> String {
    let mut characters = word.chars();
    characters
        .next()
        .map(|first_character| first_character.to_uppercase().chain(characters).collect())
        .unwrap_or_default()
}

struct FeatureGroups<'features> {
    by_leading_word: BTreeMap<&'features str, Vec<&'features str>>,
    ungrouped: Vec<&'features str>,
}

fn features_grouped_by_leading_word(features: &[String]) -> FeatureGroups<'_> {
    let mut leading_word_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for feature in features {
        let leading_word = feature
            .split_once('-')
            .map_or(feature.as_str(), |(word, _)| word);
        *leading_word_counts.entry(leading_word).or_default() += 1;
    }
    let mut feature_groups = FeatureGroups {
        by_leading_word: BTreeMap::new(),
        ungrouped: Vec::new(),
    };
    for feature in features {
        let (leading_word, remainder) = feature
            .split_once('-')
            .unwrap_or((feature.as_str(), BARE_WORD_TEXT));
        if leading_word_counts[leading_word] >= SHARED_WORD_MINIMUM {
            feature_groups
                .by_leading_word
                .entry(leading_word)
                .or_default()
                .push(remainder);
        } else {
            feature_groups.ungrouped.push(feature);
        }
    }
    feature_groups
}

fn capability_rows(capabilities: &NodeCapabilities) -> Vec<LabeledValue> {
    let feature_groups = features_grouped_by_leading_word(&capabilities.unscoped_features);
    let mut rows: Vec<LabeledValue> = capabilities
        .scoped_by_namespace
        .iter()
        .map(|(namespace, scoped_names)| {
            LabeledValue::new(capitalized_word(namespace), scoped_names.join(", "))
        })
        .chain(
            feature_groups
                .by_leading_word
                .iter()
                .map(|(leading_word, remainders)| {
                    LabeledValue::new(capitalized_word(leading_word), remainders.join(", "))
                }),
        )
        .collect();
    if !feature_groups.ungrouped.is_empty() {
        rows.push(LabeledValue::new(
            UNGROUPED_FEATURES_LABEL,
            feature_groups.ungrouped.join(", "),
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_sharing_a_leading_word_are_grouped() {
        let features = [
            "file-sharing",
            "https",
            "ssh",
            "ssh-behavior-v1",
            "ssh-env-vars",
            "tailnet-display-name",
            "tailnet-lock",
        ]
        .map(str::to_owned);
        let feature_groups = features_grouped_by_leading_word(&features);
        assert_eq!(
            feature_groups.by_leading_word,
            BTreeMap::from([
                ("ssh", vec!["enabled", "behavior-v1", "env-vars"]),
                ("tailnet", vec!["display-name", "lock"]),
            ])
        );
        assert_eq!(feature_groups.ungrouped, ["file-sharing", "https"]);
    }
}
