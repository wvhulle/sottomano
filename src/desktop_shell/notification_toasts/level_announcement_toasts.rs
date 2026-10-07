use crate::{desktop_shell::hover_tooltips::HoverTarget, widgets::announced_level::LevelReading};

#[derive(Debug, Clone)]
pub struct SeenLevel {
    hover_target: HoverTarget,
    external_change_count: u64,
}

pub fn seen_levels(current_levels: &[LevelReading]) -> Vec<SeenLevel> {
    current_levels
        .iter()
        .map(|reading| SeenLevel {
            hover_target: reading.hover_target.clone(),
            external_change_count: reading.level.external_change_count,
        })
        .collect()
}

pub fn levels_changed_outside_the_bar<'a>(
    current_levels: &'a [LevelReading],
    seen_levels: &[SeenLevel],
) -> Vec<&'a LevelReading> {
    current_levels
        .iter()
        .filter(|reading| {
            seen_levels.iter().any(|seen| {
                seen.hover_target == reading.hover_target
                    && seen.external_change_count != reading.level.external_change_count
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use uom::si::{f64::Ratio, ratio};

    use super::*;
    use crate::widgets::{
        announced_level::AnnouncedLevel,
        phosphor::{Phosphor, Weight},
    };

    fn reading(hover_target: HoverTarget, external_change_count: u64) -> LevelReading {
        LevelReading {
            hover_target,
            glyph: Phosphor::SpeakerHigh(Weight::Regular).into(),
            level: AnnouncedLevel {
                summary_text: "Speaker",
                readout_text: "50%".to_owned(),
                fill: Ratio::new::<ratio::ratio>(0.5),
                muted: false,
                external_change_count,
            },
        }
    }

    fn changed_targets(current: &[LevelReading], seen: &[LevelReading]) -> Vec<HoverTarget> {
        levels_changed_outside_the_bar(current, &seen_levels(seen))
            .into_iter()
            .map(|reading| reading.hover_target.clone())
            .collect()
    }

    #[test]
    fn levels_seen_for_the_first_time_are_not_announced() {
        assert!(changed_targets(&[reading(HoverTarget::AudioSink, 3)], &[]).is_empty());
    }

    #[test]
    fn unchanged_levels_are_not_announced() {
        let current = [reading(HoverTarget::AudioSink, 3)];
        assert!(changed_targets(&current, &current).is_empty());
    }

    #[test]
    fn only_the_level_that_changed_is_announced() {
        let seen = [
            reading(HoverTarget::AudioSink, 3),
            reading(HoverTarget::AudioSource, 1),
        ];
        let current = [
            reading(HoverTarget::AudioSink, 4),
            reading(HoverTarget::AudioSource, 1),
        ];
        assert_eq!(changed_targets(&current, &seen), [HoverTarget::AudioSink]);
    }
}
