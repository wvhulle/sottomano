use corredo::gauges::DurationTimeline;
use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction, system_hibernate_delay::HibernateDelay,
};
use sottomano_toml_config::Config;

use super::{
    IdlePauseGroups, IdlePauseInteraction, StageSchedule,
    chain_stage_rows::{
        HibernateDelayApply, hibernate_delay_failure_row, system_hibernate_delay_row,
    },
    idle_timeline_row::{first_stage_text, idle_timeline},
    pause_blocker_indicators::{
        PauseBlocker, ScheduledShutdownIndicator, blocker_sections, sleep_state_sections,
    },
    stage_schedule_cards::{ScreenOffOverride, schedule_section},
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState, slider_control::SliderActivity,
    },
    hover_cards::hover_card_body::{Section, hover_card_figure_payloads::timeline_widget},
    indicator_groups::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::{idle::timer::IdleTimer, text::text_face_of},
};

const IDLE_CHAIN_INSTANCE: &str = "chain";
const IDLE_CHAIN_TITLE: &str = "Idle";
const STAGE_SCHEDULE_TITLE: &str = "Schedule";
const TIMELINE_SECTION_TITLE: &str = "From last input";

pub(super) fn pause_glyph(pause: IdleAction) -> Phosphor {
    match pause {
        IdleAction::Lock => Phosphor::Lock(Weight::Regular),
        IdleAction::ScreenOff => Phosphor::Monitor(Weight::Regular),
        IdleAction::Sleep => Phosphor::Bed(Weight::Regular),
        IdleAction::Hibernate => Phosphor::Snowflake(Weight::Regular),
        IdleAction::PowerOff => Phosphor::Power(Weight::Regular),
    }
}

pub(super) fn stage_slider_glyph(stage: IdleAction) -> fn(f32, SliderActivity) -> Phosphor {
    match stage {
        IdleAction::Lock => |_, activity| Phosphor::Lock(activity.into()),
        IdleAction::ScreenOff => |_, activity| Phosphor::Monitor(activity.into()),
        IdleAction::Sleep => |_, activity| Phosphor::Bed(activity.into()),
        IdleAction::Hibernate => |_, activity| Phosphor::Snowflake(activity.into()),
        IdleAction::PowerOff => |_, activity| Phosphor::Power(activity.into()),
    }
}

fn headline_rank(state: IndicatorState) -> u8 {
    match state {
        IndicatorState::Critical | IndicatorState::Failed => 2,
        IndicatorState::Warning | IndicatorState::Pending => 1,
        IndicatorState::Idle | IndicatorState::Off | IndicatorState::On | IndicatorState::Live => 0,
    }
}

impl IdlePauseGroups {
    fn screen_off_override(&self, kind: IdleAction) -> ScreenOffOverride {
        match self.quiet_screen_off {
            Some(quiet_offset) if kind == IdleAction::ScreenOff => {
                ScreenOffOverride::QuietHours(quiet_offset)
            }
            _ => ScreenOffOverride::FollowsSlider,
        }
    }

    fn stage_schedule_section(
        &self,
        section_title: &'static str,
        kind: IdleAction,
        clock: Clock,
    ) -> Section {
        schedule_section(
            section_title,
            self.chain_of(IdleTimer::shown_setting).stage(kind),
            self.screen_off_override(kind),
            self.hibernate_delay.get().copied(),
            clock,
        )
    }

    fn pause_sections(&self, kind: IdleAction, clock: Clock) -> Vec<Section> {
        [self.stage_schedule_section(STAGE_SCHEDULE_TITLE, kind, clock)]
            .into_iter()
            .chain(self.sleep_obstacles.sections_described_by(|obstacles| {
                let sleep_states = if kind == IdleAction::Sleep {
                    sleep_state_sections(obstacles)
                } else {
                    Vec::new()
                };
                sleep_states
                    .into_iter()
                    .chain(blocker_sections(self.blockers_of(kind)))
                    .collect()
            }))
            .collect()
    }

    fn shown_idle_timeline(&self, config: &Config, clock: Clock) -> DurationTimeline {
        idle_timeline(
            &self.chain_of(IdleTimer::shown_setting),
            clock,
            config.calendar.locale,
            text_face_of(&config.theme.font),
        )
    }

    fn timeline_sections(&self, config: &Config, clock: Clock) -> Vec<Section> {
        vec![Section::new(
            TIMELINE_SECTION_TITLE,
            timeline_widget(self.shown_idle_timeline(config, clock)),
        )]
    }

    fn blocker_group_sections(&self, pause: IdleAction) -> Vec<Section> {
        self.sleep_obstacles
            .hover_card_with(blocker_sections(self.blockers_of(pause)))
    }

    fn hibernate_delay_rows<'a>(&'a self, config: &'a Config) -> Vec<GroupIndicator<'a>> {
        let StageSchedule::AsleepAfterStandby { asleep_for } =
            self.idle_chain().stage(IdleAction::Hibernate).schedule
        else {
            return Vec::new();
        };
        let section_card = move |card_clock| self.pause_sections(IdleAction::Hibernate, card_clock);
        let mut rows = Vec::new();
        if let Some(system_delay) = self.hibernate_delay.get()
            && !asleep_for.is_zero()
            && (*system_delay != HibernateDelay::Configured(asleep_for)
                || self.hibernate_delay_apply == HibernateDelayApply::Applying)
        {
            rows.push(
                system_hibernate_delay_row(
                    asleep_for,
                    *system_delay,
                    &self.hibernate_delay_apply,
                    config.calendar.locale,
                )
                .with_hover_card(Box::new(section_card)),
            );
        }
        if let HibernateDelayApply::Failed(failure_text) = &self.hibernate_delay_apply {
            rows.push(
                hibernate_delay_failure_row(failure_text).with_hover_card(Box::new(section_card)),
            );
        }
        rows
    }

    fn stage_rows<'a>(
        &'a self,
        pause: IdleAction,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<GroupIndicator<'a>> {
        let timeout_slider =
            GroupIndicator::slider(&self.pause(pause).timer.control, config, move |message| {
                IdlePauseInteraction::Timer(pause, message)
            })
            .with_hover_card(Box::new(move |card_clock| {
                self.pause_sections(pause, card_clock)
            }));
        let mut rows = vec![timeout_slider];
        rows.extend(self.blockers_of(pause).map(move |blocker| {
            blocker
                .indicator()
                .with_hover_card(Box::new(move |_| self.blocker_group_sections(pause)))
        }));
        if pause == IdleAction::Hibernate {
            rows.extend(self.hibernate_delay_rows(config));
        }
        if pause == IdleAction::PowerOff
            && let Some(shutdown) = self
                .sleep_obstacles
                .get()
                .and_then(|obstacles| obstacles.scheduled_shutdown.as_ref())
        {
            rows.push(
                ScheduledShutdownIndicator { shutdown }
                    .indicator(clock, config.calendar.locale)
                    .with_hover_card(Box::new(move |card_clock| {
                        self.pause_sections(IdleAction::PowerOff, card_clock)
                    })),
            );
        }
        rows
    }

    fn timeline_indicator<'a>(&'a self, config: &'a Config) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            "Timeline",
            Phosphor::ClockCountdown(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_label(ReadoutLabel::Labeled(first_stage_text(
            &self.chain_of(IdleTimer::shown_setting),
            config.calendar.locale,
        )))
        .with_hover_card(Box::new(move |card_clock| {
            self.timeline_sections(config, card_clock)
        }))
    }

    fn headline_state(&self) -> IndicatorState {
        self.pause_blockers
            .iter()
            .map(PauseBlocker::display_state)
            .max_by_key(|state| headline_rank(*state))
            .unwrap_or(IndicatorState::Idle)
    }

    #[must_use]
    pub fn idle_chain_group<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'a> {
        let rows = [self.timeline_indicator(config)].into_iter().chain(
            IdleAction::ALL
                .into_iter()
                .flat_map(|pause| self.stage_rows(pause, config, clock)),
        );
        IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::IdleTimeouts,
                IDLE_CHAIN_INSTANCE,
            ),
            IndicatorGroupTitle::fixed(IDLE_CHAIN_TITLE),
            GroupIndicator::glyph_only(
                IDLE_CHAIN_TITLE,
                Phosphor::Moon(Weight::Regular).into(),
                self.headline_state(),
            ),
            &self.expansion,
            IdlePauseInteraction::Expansion,
            rows.collect::<Vec<_>>(),
        )
    }
}
