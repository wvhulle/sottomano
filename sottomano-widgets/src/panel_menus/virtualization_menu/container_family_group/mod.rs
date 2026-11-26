use std::{
    fmt::Debug,
    future::{Future, ready},
};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::sink_write_errors::GuestPowerError;
use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub enum RowPower<PowerRequest> {
    SwitchedOn(PowerRequest),
    SwitchedOff(PowerRequest),
    Stoppable(PowerRequest),
}

pub struct FamilyIndicator<'view, Family: ContainerFamily> {
    pub indicator: GroupIndicator<'view>,
    pub power: RowPower<Family::PowerRequest>,
}

pub trait ContainerFamily: Debug + Clone + Sized + 'static {
    type Probe: SnapshotSource<StreamInputs = ()>;
    type Listings: SnapshotSource<StreamInputs = ()>;
    type Detail: SnapshotSource<StreamInputs = ()>;
    type PowerRequest: Debug + Clone + Send + 'static;

    const FAMILY: IndicatorGroupFamily;
    const GROUP_NAME: &'static str;
    const STOP_HINT: &'static str;
    const START_HINT: &'static str = "start";

    fn group_glyph() -> IconGlyph;

    fn running_count(probe: &Self::Probe) -> usize;

    fn listing_indicators(listings: &Self::Listings) -> Vec<FamilyIndicator<'_, Self>>;

    fn detail_indicators(detail: &Self::Detail) -> Vec<FamilyIndicator<'_, Self>>;

    fn stopped_indicators(_listings: &Self::Listings) -> Vec<FamilyIndicator<'_, Self>> {
        Vec::new()
    }

    fn stop(
        request: Self::PowerRequest,
    ) -> impl Future<Output = Result<(), GuestPowerError>> + Send + 'static;

    fn start(
        request: Self::PowerRequest,
    ) -> impl Future<Output = Result<(), GuestPowerError>> + Send + 'static {
        ready(Err(GuestPowerError::StartUnsupported {
            group_name: Self::GROUP_NAME,
            request: format!("{request:?}"),
        }))
    }
}

pub struct ContainerFamilyGroup<Family: ContainerFamily> {
    probe: SubscribedSnapshot<Family::Probe>,
    listings: SubscribedSnapshot<Family::Listings>,
    detail: SubscribedSnapshot<Family::Detail>,
    expansion: IndicatorGroupExpansion,
}

impl<Family: ContainerFamily> Default for ContainerFamilyGroup<Family> {
    fn default() -> Self {
        Self {
            probe: SubscribedSnapshot::default(),
            listings: SubscribedSnapshot::default(),
            detail: SubscribedSnapshot::default(),
            expansion: IndicatorGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum ContainerFamilyMessage<Family: ContainerFamily> {
    Probe(SnapshotDelivery<Family::Probe>),
    Listings(SnapshotDelivery<Family::Listings>),
    Detail(SnapshotDelivery<Family::Detail>),
    PowerChangeRefused(GuestPowerError),
}

#[derive(Debug, Clone)]
pub enum ContainerFamilyInteraction<Family: ContainerFamily> {
    Stop(Family::PowerRequest),
    Start(Family::PowerRequest),
    Expansion(IndicatorGroupExpansionMessage),
}

impl<Family: ContainerFamily> ContainerFamilyGroup<Family>
where
    ContainerFamilyInteraction<Family>: Into<IndicatorGroupInteraction>,
{
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        demand: GroupSourceDemand,
    ) -> Subscription<ContainerFamilyMessage<Family>> {
        let demand = demand.expanded_when(self.expansion.state);
        Subscription::batch([
            snapshot_subscription::<Family::Probe>(()).map(ContainerFamilyMessage::Probe),
            demand
                .while_headline_shown(snapshot_subscription::<Family::Listings>(()))
                .map(ContainerFamilyMessage::Listings),
            demand
                .while_indicators_shown(snapshot_subscription::<Family::Detail>(()))
                .map(ContainerFamilyMessage::Detail),
        ])
    }

    pub fn update(
        &mut self,
        message: ContainerFamilyMessage<Family>,
        clock: Clock,
    ) -> Task<ContainerFamilyMessage<Family>> {
        match message {
            ContainerFamilyMessage::Probe(delivery) => self.probe.receive(delivery, clock),
            ContainerFamilyMessage::Listings(delivery) => self.listings.receive(delivery, clock),
            ContainerFamilyMessage::Detail(delivery) => self.detail.receive(delivery, clock),
            ContainerFamilyMessage::PowerChangeRefused(refusal) => {
                tracing::warn!(group = Family::GROUP_NAME, %refusal, "power change refused");
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: ContainerFamilyInteraction<Family>,
    ) -> Task<ContainerFamilyMessage<Family>> {
        match interaction {
            ContainerFamilyInteraction::<Family>::Stop(request) => {
                refused_power_change(Family::stop(request))
            }
            ContainerFamilyInteraction::<Family>::Start(request) => {
                refused_power_change(Family::start(request))
            }
            ContainerFamilyInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn detail_indicators(&self) -> Vec<FamilyIndicator<'_, Family>> {
        self.detail
            .get()
            .map(|detail| Family::detail_indicators(detail))
            .unwrap_or_default()
    }

    fn indicator_listing(&self) -> IndicatorListing<'_> {
        let listed_indicator = |row| switchable_indicator(stamped_by_source(&self.listings, row));
        let detail_indicators = self.detail_indicators();
        if detail_indicators.is_empty() {
            return self.listings.indicator_listing(
                |listings| {
                    Family::listing_indicators(listings)
                        .into_iter()
                        .chain(Family::stopped_indicators(listings))
                        .map(listed_indicator)
                        .collect()
                },
                Family::group_glyph(),
            );
        }
        let stopped_indicators = self
            .listings
            .get()
            .map(|listings| Family::stopped_indicators(listings))
            .unwrap_or_default();
        IndicatorListing::Listed(
            detail_indicators
                .into_iter()
                .map(|row| switchable_indicator(stamped_by_source(&self.detail, row)))
                .chain(stopped_indicators.into_iter().map(listed_indicator))
                .collect(),
        )
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            Family::FAMILY,
            GroupIndicator::glyph_only(
                Family::FAMILY.title(),
                Family::group_glyph(),
                match self.probe.get() {
                    Some(probe) if Family::running_count(probe) > 0 => IndicatorState::On,
                    Some(_) | None => IndicatorState::Off,
                },
            ),
            &self.expansion,
            ContainerFamilyInteraction::Expansion,
            self.indicator_listing(),
        )
    }
}

fn refused_power_change<Family: ContainerFamily>(
    power_change: impl Future<Output = Result<(), GuestPowerError>> + Send + 'static,
) -> Task<ContainerFamilyMessage<Family>> {
    Task::future(async move { power_change.await.err() })
        .and_then(|refusal| Task::done(ContainerFamilyMessage::PowerChangeRefused(refusal)))
}

fn stamped_by_source<'view, Family: ContainerFamily, Source: SnapshotSource>(
    source: &'view SubscribedSnapshot<Source>,
    row: FamilyIndicator<'view, Family>,
) -> FamilyIndicator<'view, Family> {
    FamilyIndicator {
        indicator: row.indicator.wrap_hover_card(|indicator_card| {
            Box::new(move |clock| source.hover_card_with(indicator_card(clock)))
        }),
        ..row
    }
}

fn switchable_indicator<Family: ContainerFamily>(
    row: FamilyIndicator<'_, Family>,
) -> GroupIndicator<'_>
where
    ContainerFamilyInteraction<Family>: Into<IndicatorGroupInteraction>,
{
    let indicator = row.indicator;
    match row.power {
        RowPower::SwitchedOn(request) => indicator.on_switch_press(
            SwitchPosition::On,
            Family::STOP_HINT,
            ContainerFamilyInteraction::<Family>::Stop(request),
        ),
        RowPower::SwitchedOff(request) => indicator.on_switch_press(
            SwitchPosition::Off,
            Family::START_HINT,
            ContainerFamilyInteraction::<Family>::Start(request),
        ),
        RowPower::Stoppable(request) => indicator.on_stop_press(
            Family::STOP_HINT,
            ContainerFamilyInteraction::<Family>::Stop(request),
        ),
    }
}

#[cfg(test)]
mod container_family_group_tests;
