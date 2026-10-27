use std::collections::BTreeSet;

const LIT_LEDS_PER_PLAYER: [&[u8]; 5] =
    [&[3], &[2, 4], &[1, 3, 5], &[1, 2, 4, 5], &[1, 2, 3, 4, 5]];

#[must_use]
pub fn playstation_player_number(lit_leds: &BTreeSet<u8>) -> Option<u8> {
    LIT_LEDS_PER_PLAYER
        .iter()
        .position(|pattern| pattern.iter().copied().eq(lit_leds.iter().copied()))
        .and_then(|index| u8::try_from(index + 1).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_middle_led_alone_is_player_one() {
        assert_eq!(playstation_player_number(&BTreeSet::from([3])), Some(1));
    }

    #[test]
    fn the_two_inner_leds_are_player_two() {
        assert_eq!(playstation_player_number(&BTreeSet::from([2, 4])), Some(2));
    }

    #[test]
    fn an_unknown_pattern_has_no_player() {
        assert_eq!(playstation_player_number(&BTreeSet::from([1])), None);
        assert_eq!(playstation_player_number(&BTreeSet::new()), None);
    }
}
