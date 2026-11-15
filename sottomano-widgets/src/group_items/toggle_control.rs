use std::borrow::Cow;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::{
        network_manager_proxies::{LinkPhase, RadioPower},
        tailscale_exit_nodes::ExitNodeLanAccess,
        tailscale_status::ExitNodeUse,
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem, reading_display_state::ReadingDisplayState, reading_row::ReadingRow,
    },
    hover_cards::{HoverTarget, hover_card_body::Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::{CommandOutcome, SubscribedSnapshot},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchPosition {
    On,
    Off,
}

impl From<SwitchPosition> for ReadingDisplayState {
    fn from(switch_position: SwitchPosition) -> Self {
        match switch_position {
            SwitchPosition::On => ReadingDisplayState::On,
            SwitchPosition::Off => ReadingDisplayState::Off,
        }
    }
}

impl From<LinkPhase> for SwitchPosition {
    fn from(link_state: LinkPhase) -> Self {
        match link_state {
            LinkPhase::Up => SwitchPosition::On,
            LinkPhase::Switching | LinkPhase::Down => SwitchPosition::Off,
        }
    }
}

impl From<RadioPower> for SwitchPosition {
    fn from(radio_power: RadioPower) -> Self {
        match radio_power {
            RadioPower::On => SwitchPosition::On,
            RadioPower::Off => SwitchPosition::Off,
        }
    }
}

impl From<ExitNodeUse> for SwitchPosition {
    fn from(node_use: ExitNodeUse) -> Self {
        match node_use {
            ExitNodeUse::InUse => SwitchPosition::On,
            ExitNodeUse::Unused => SwitchPosition::Off,
        }
    }
}

impl From<ExitNodeLanAccess> for SwitchPosition {
    fn from(lan_access: ExitNodeLanAccess) -> Self {
        match lan_access {
            ExitNodeLanAccess::Allowed => SwitchPosition::On,
            ExitNodeLanAccess::Blocked => SwitchPosition::Off,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SwitchPresses {
    pub turning_on: &'static str,
    pub turning_off: &'static str,
}

#[derive(Clone)]
pub struct IconToggle {
    pub name: Cow<'static, str>,
    pub icon: Phosphor,
    pub switch_position: SwitchPosition,
    pub presses: SwitchPresses,
}

impl IconToggle {
    #[must_use]
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        icon: Phosphor,
        switch_position: impl Into<SwitchPosition>,
        presses: SwitchPresses,
    ) -> Self {
        Self {
            name: name.into(),
            icon,
            switch_position: switch_position.into(),
            presses,
        }
    }

    #[must_use]
    pub fn press_does(&self) -> &'static str {
        match self.switch_position {
            SwitchPosition::On => self.presses.turning_off,
            SwitchPosition::Off => self.presses.turning_on,
        }
    }

    #[must_use]
    pub fn reading_row(self) -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            self.name,
            self.icon.with_weight(Weight::Regular).into(),
            ReadingDisplayState::from(self.switch_position),
            Box::new(|_| Vec::new()),
        )
    }
}

pub trait ToggleableSource: SnapshotSource {
    #[must_use]
    fn toggled_value(&self) -> Self;
    fn apply_toggle(&self) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static;
    fn press_action_text(&self) -> &'static str;
    fn switch_position(&self) -> SwitchPosition;
}

pub struct ToggleControl<S: ToggleableSource> {
    reading: SubscribedSnapshot<S>,
}

impl<S: ToggleableSource> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            reading: SubscribedSnapshot::default(),
        }
    }
}

#[derive(Debug)]
pub enum ToggleMessage<S: ToggleableSource> {
    Reading(SnapshotDelivery<S>),
    Applied,
    Refused(S::Error),
}

impl<S: ToggleableSource> ToggleControl<S> {
    pub fn subscription(inputs: S::StreamInputs) -> Subscription<ToggleMessage<S>> {
        snapshot_subscription::<S>(inputs).map(ToggleMessage::Reading)
    }

    pub fn refresh(inputs: S::StreamInputs) -> Task<ToggleMessage<S>> {
        first_delivery::<S>(inputs).map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>, clock: Clock) -> Task<ToggleMessage<S>> {
        match message {
            ToggleMessage::Reading(reading) => {
                self.reading.receive(reading, clock);
                Task::none()
            }
            ToggleMessage::Applied => Task::none(),
            ToggleMessage::Refused(refusal) => {
                self.reading
                    .apply_command(CommandOutcome::Refused { refusal });
                Task::none()
            }
        }
    }

    pub fn press(&mut self) -> Task<ToggleMessage<S>> {
        let Some(current) = self.reading.get().cloned() else {
            return Task::none();
        };
        self.reading.apply_command(CommandOutcome::Applied {
            expected_snapshot: current.toggled_value(),
        });
        Task::future(current.apply_toggle()).map(|applied| match applied {
            Ok(()) => ToggleMessage::Applied,
            Err(refusal) => ToggleMessage::Refused(refusal),
        })
    }

    pub fn apply_command(&mut self, command_outcome: CommandOutcome<S>) {
        self.reading.apply_command(command_outcome);
    }

    pub fn hover_card(&self) -> Vec<Section> {
        self.reading.hover_card_with(Vec::new())
    }

    pub fn snapshot(&self) -> Option<&S> {
        self.reading.get()
    }

    pub fn group_item<'s, M: Clone + 's>(
        &'s self,
        fallback_icon: IconGlyph,
        describe: impl FnOnce(&'s S) -> ReadingRow<'s>,
        target: impl Into<HoverTarget>,
        on_press: M,
        clock: Clock,
    ) -> Option<GroupItem<'s, M>> {
        let shown = self
            .reading
            .group_item(fallback_icon, describe, target, clock)?;
        Some(match self.reading.get() {
            Some(current) => shown.on_switch_press(
                current.switch_position(),
                current.press_action_text(),
                on_press,
            ),
            None => shown,
        })
    }
}

#[cfg(test)]
mod tests {

    use sottomano_desktop_sources::{
        cpu_memory_compute::amd_x3d_cache_preference::X3dCachePreference,
        system_protocol_clients::sysfs_attribute_reads::FileReadError,
    };

    use super::*;
    use crate::{
        hover_cards::hover_card_body::hover_card_source_delivery::{SectionSource, SourceDelivery},
        widget_groups::compute_group::{
            ComputeTarget, x3d_cache_preference_toggle::x3d_cache_preference_reading_row,
        },
    };

    fn refused_write() -> FileReadError {
        FileReadError::Unwritable {
            path: "/sys/bus/platform/drivers/amd_x3d_vcache/AMDI0101:00/amd_x3d_mode".to_owned(),
            detail: "Permission denied (os error 13)".to_owned(),
        }
    }

    #[test]
    fn a_refused_write_shows_the_refusal_instead_of_the_assumed_value() {
        let clock = Clock::real();
        let mut control = ToggleControl::<X3dCachePreference>::default();
        control.reading.receive(
            SnapshotDelivery::single_read(Ok(X3dCachePreference::Frequency)),
            Clock::real(),
        );
        control.apply_command(CommandOutcome::Applied {
            expected_snapshot: X3dCachePreference::Cache,
        });

        let _ = control.update(ToggleMessage::Refused(refused_write()), Clock::real());

        assert!(control.snapshot().is_none());
        let shown = control
            .reading
            .reading_row(
                IconGlyph::default(),
                |snapshot| x3d_cache_preference_reading_row(snapshot),
                clock,
            )
            .expect("failed reading still shows");
        assert_eq!(shown.shown_state(), ReadingDisplayState::Failed);
        assert!(
            control
                .group_item(
                    IconGlyph::default(),
                    |snapshot| x3d_cache_preference_reading_row(snapshot),
                    HoverTarget::Compute(ComputeTarget::X3dCachePreference),
                    (),
                    clock,
                )
                .is_some()
        );
        let shown_sections: Vec<(String, SectionSource)> = control
            .hover_card()
            .iter()
            .map(|section| (section.title().text().to_owned(), section.source()))
            .collect();
        assert_eq!(
            shown_sections,
            [(
                "Unavailable".to_owned(),
                SectionSource::Snapshot {
                    source_name: X3dCachePreference::NAME,
                    delivery: SourceDelivery::Failed,
                }
            )]
        );
    }

    #[test]
    fn an_accepted_write_keeps_the_assumed_value() {
        let mut control = ToggleControl::<X3dCachePreference>::default();
        control.reading.receive(
            SnapshotDelivery::single_read(Ok(X3dCachePreference::Frequency)),
            Clock::real(),
        );
        control.apply_command(CommandOutcome::Applied {
            expected_snapshot: X3dCachePreference::Cache,
        });

        let _ = control.update(ToggleMessage::Applied, Clock::real());

        assert_eq!(control.reading.get(), Some(&X3dCachePreference::Cache));
    }
}
