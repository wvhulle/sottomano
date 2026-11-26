use crate::group_indicators::{
    aligned_quantity::IndicatorAlignment, group_indicator::GroupIndicator,
};

pub enum IndicatorListing<'view> {
    Listed(Vec<GroupIndicator<'view>>),
    Incomplete {
        listed_indicators: Vec<GroupIndicator<'view>>,
        unread_sources: Vec<GroupIndicator<'view>>,
    },
}

impl<'view> IndicatorListing<'view> {
    #[must_use]
    pub fn listed_indicators(&self) -> &[GroupIndicator<'view>] {
        match self {
            IndicatorListing::Listed(listed_indicators)
            | IndicatorListing::Incomplete {
                listed_indicators, ..
            } => listed_indicators,
        }
    }

    #[must_use]
    pub fn unread_sources(&self) -> &[GroupIndicator<'view>] {
        match self {
            IndicatorListing::Listed(_) => &[],
            IndicatorListing::Incomplete { unread_sources, .. } => unread_sources,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &GroupIndicator<'view>> {
        self.listed_indicators().iter().chain(self.unread_sources())
    }

    #[must_use]
    pub fn with_listed(
        self,
        transform_listed: impl FnOnce(Vec<GroupIndicator<'view>>) -> Vec<GroupIndicator<'view>>,
    ) -> Self {
        match self {
            IndicatorListing::Listed(listed_indicators) => {
                IndicatorListing::Listed(transform_listed(listed_indicators))
            }
            IndicatorListing::Incomplete {
                listed_indicators,
                unread_sources,
            } => IndicatorListing::Incomplete {
                listed_indicators: transform_listed(listed_indicators),
                unread_sources,
            },
        }
    }

    #[must_use]
    pub fn map_indicators(
        self,
        transform_indicator: impl Fn(GroupIndicator<'view>) -> GroupIndicator<'view>,
    ) -> Self {
        match self {
            IndicatorListing::Listed(listed_indicators) => IndicatorListing::Listed(
                listed_indicators
                    .into_iter()
                    .map(&transform_indicator)
                    .collect(),
            ),
            IndicatorListing::Incomplete {
                listed_indicators,
                unread_sources,
            } => IndicatorListing::Incomplete {
                listed_indicators: listed_indicators
                    .into_iter()
                    .map(&transform_indicator)
                    .collect(),
                unread_sources: unread_sources
                    .into_iter()
                    .map(&transform_indicator)
                    .collect(),
            },
        }
    }

    pub(crate) fn retaining_listed(
        self,
        keeps_alignment: impl Fn(IndicatorAlignment) -> bool,
    ) -> Self {
        let keeps_indicator = |indicator: &GroupIndicator<'_>| keeps_alignment(indicator.alignment);
        match self {
            IndicatorListing::Listed(listed_indicators) => IndicatorListing::Listed(
                listed_indicators
                    .into_iter()
                    .filter(keeps_indicator)
                    .collect(),
            ),
            IndicatorListing::Incomplete {
                listed_indicators,
                unread_sources,
            } => IndicatorListing::Incomplete {
                listed_indicators: listed_indicators
                    .into_iter()
                    .filter(keeps_indicator)
                    .collect(),
                unread_sources,
            },
        }
    }

    fn followed_by(self, next_listing: IndicatorListing<'view>) -> Self {
        match (self, next_listing) {
            (IndicatorListing::Listed(mut combined_listed), IndicatorListing::Listed(listed)) => {
                combined_listed.extend(listed);
                IndicatorListing::Listed(combined_listed)
            }
            (
                IndicatorListing::Listed(mut combined_listed),
                IndicatorListing::Incomplete {
                    listed_indicators,
                    unread_sources,
                },
            ) => {
                combined_listed.extend(listed_indicators);
                IndicatorListing::Incomplete {
                    listed_indicators: combined_listed,
                    unread_sources,
                }
            }
            (
                IndicatorListing::Incomplete {
                    mut listed_indicators,
                    unread_sources,
                },
                IndicatorListing::Listed(listed),
            ) => {
                listed_indicators.extend(listed);
                IndicatorListing::Incomplete {
                    listed_indicators,
                    unread_sources,
                }
            }
            (
                IndicatorListing::Incomplete {
                    mut listed_indicators,
                    mut unread_sources,
                },
                IndicatorListing::Incomplete {
                    listed_indicators: next_listed,
                    unread_sources: next_unread,
                },
            ) => {
                listed_indicators.extend(next_listed);
                unread_sources.extend(next_unread);
                IndicatorListing::Incomplete {
                    listed_indicators,
                    unread_sources,
                }
            }
        }
    }
}

impl<'view> FromIterator<IndicatorListing<'view>> for IndicatorListing<'view> {
    fn from_iter<Listings: IntoIterator<Item = IndicatorListing<'view>>>(
        listings: Listings,
    ) -> Self {
        listings.into_iter().fold(
            IndicatorListing::Listed(Vec::new()),
            IndicatorListing::followed_by,
        )
    }
}

#[cfg(test)]
mod indicator_listing_tests;
