use iced::{Subscription, Task};
use sottomano_sources::{ReadingSource, SourceDelivery, blocking_background_work::blocking};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Freshness, Section, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

#[derive(Clone, Copy)]
pub struct SwitchPresses {
    pub turning_on: &'static str,
    pub turning_off: &'static str,
}

#[derive(Clone, Copy)]
pub struct IconToggle {
    pub name: &'static str,
    pub icon: Phosphor,
    pub enabled: bool,
    pub presses: SwitchPresses,
}

impl IconToggle {
    #[must_use]
    pub fn new(name: &'static str, icon: Phosphor, enabled: bool, presses: SwitchPresses) -> Self {
        Self {
            name,
            icon,
            enabled,
            presses,
        }
    }

    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.enabled {
            self.presses.turning_off
        } else {
            self.presses.turning_on
        }
    }

    #[must_use]
    pub fn switch_tooltip(&self, clock: Clock) -> Tooltip {
        Tooltip::sourced(self.tooltip(clock), Freshness::Live)
    }
}

impl BarWidget for IconToggle {
    fn name(&self) -> &'static str {
        self.name
    }

    fn icon(&self) -> Glyph {
        self.icon.with_weight(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(self.enabled)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

pub trait ToggleableSource: ReadingSource<Error: Into<Section>> + BarWidget {
    #[must_use]
    fn toggled_value(&self) -> Self;
    fn apply_toggle(&self) -> Result<(), Self::Error>;
    fn press_action_text(&self) -> &'static str;
}

pub struct ToggleControl<S: ReadingSource> {
    reading: ContinuousReading<S>,
}

impl<S: ReadingSource> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            reading: ContinuousReading::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMessage<S: ToggleableSource> {
    Reading(SourceDelivery<S>),
    Press,
    Applied,
    Refused(S::Error),
}

impl<S: ToggleableSource> ToggleControl<S> {
    pub fn subscription(inputs: S::StreamInputs) -> Subscription<ToggleMessage<S>> {
        source_subscription::<S>(inputs).map(ToggleMessage::Reading)
    }

    pub fn refresh(inputs: S::StreamInputs) -> Task<ToggleMessage<S>> {
        first_delivery::<S>(inputs).map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>, clock: Clock) -> Task<ToggleMessage<S>> {
        match message {
            ToggleMessage::Reading(reading) => {
                self.reading.receive_delivery(reading, clock);
                Task::none()
            }
            ToggleMessage::Press => match self.reading.get().cloned() {
                Some(current) => {
                    self.assume(current.toggled_value());
                    Task::future(blocking(S::NAME, move || current.apply_toggle().err())).map(
                        |refusal| refusal.map_or(ToggleMessage::Applied, ToggleMessage::Refused),
                    )
                }
                None => Task::none(),
            },
            ToggleMessage::Applied => Task::none(),
            ToggleMessage::Refused(refusal) => {
                self.reading
                    .receive_delivery(SourceDelivery::Reading(Err(refusal)), clock);
                Task::none()
            }
        }
    }

    pub fn assume(&mut self, value: S) {
        self.reading.assume(value);
    }

    pub fn tooltip(&self, clock: Clock) -> Option<Tooltip> {
        match self.reading.get() {
            Some(_) => self.reading.tooltip_with(TooltipBody::sections([])),
            None => self.reading.tooltip(clock),
        }
    }

    pub fn reading(&self) -> Option<&dyn BarWidget> {
        self.reading.get().map(|display| display as &dyn BarWidget)
    }

    pub fn group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
        on_toggle: fn(ToggleMessage<S>) -> M,
    ) -> Option<GroupItem<'_, M>> {
        let shown = self.reading.group_item(target)?;
        Some(match self.reading.get() {
            Some(current) => {
                shown.on_press(current.press_action_text(), on_toggle(ToggleMessage::Press))
            }
            None => shown,
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::{
        cpu_memory_compute::amd_x3d_cache_preference::X3dCachePreference,
        system_protocol_clients::sysfs_attribute_reads::Error,
    };

    use super::*;
    use crate::widget_groups::compute_group::ComputeTarget;

    fn refused_write() -> Error {
        Error::Unwritable {
            path: "/sys/bus/platform/drivers/amd_x3d_vcache/AMDI0101:00/amd_x3d_mode".to_owned(),
            detail: "Permission denied (os error 13)".to_owned(),
        }
    }

    #[test]
    fn a_refused_write_shows_the_refusal_instead_of_the_assumed_value() {
        let mut control = ToggleControl::<X3dCachePreference>::default();
        control
            .reading
            .load(X3dCachePreference::Frequency, Clock::real());
        control.assume(X3dCachePreference::Cache);

        let _ = control.update(ToggleMessage::Refused(refused_write()), Clock::real());

        assert!(control.reading().is_none());
        let shown = control
            .reading
            .display()
            .expect("failed reading still shows");
        assert_eq!(shown.state(Clock::real()), WidgetState::Failed);
        assert!(
            control
                .group_item::<()>(
                    HoverTarget::Compute(ComputeTarget::X3dCachePreference),
                    |_| ()
                )
                .is_some()
        );
        let hover_card = format!(
            "{:?}",
            control.tooltip(Clock::real()).map(|tooltip| tooltip.body)
        );
        assert!(hover_card.contains(&refused_write().to_string()));
    }

    #[test]
    fn an_accepted_write_keeps_the_assumed_value() {
        let mut control = ToggleControl::<X3dCachePreference>::default();
        control
            .reading
            .load(X3dCachePreference::Frequency, Clock::real());
        control.assume(X3dCachePreference::Cache);

        let _ = control.update(ToggleMessage::Applied, Clock::real());

        assert_eq!(control.reading.get(), Some(&X3dCachePreference::Cache));
    }
}
