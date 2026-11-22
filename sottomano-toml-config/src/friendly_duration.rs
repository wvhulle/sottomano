use std::time::Duration;

use jiff::{
    Span, SpanRelativeTo,
    fmt::friendly::{Designator, SpanParser, SpanPrinter},
};
use serde::{Deserialize, Deserializer, Serializer, de::Error as DeserializeError};

static FRIENDLY_SPAN_PARSER: SpanParser = SpanParser::new();
static COMPACT_SPAN_PRINTER: SpanPrinter = SpanPrinter::new().designator(Designator::Compact);

pub fn parsed_friendly_duration(written_length: &str) -> Result<Duration, jiff::Error> {
    let written_span: Span = FRIENDLY_SPAN_PARSER.parse_span(written_length)?;
    Duration::try_from(written_span.to_duration(SpanRelativeTo::days_are_24_hours())?)
}

#[must_use]
pub fn friendly_duration_text(length: Duration) -> String {
    COMPACT_SPAN_PRINTER.unsigned_duration_to_string(&length)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    parsed_friendly_duration(&String::deserialize(deserializer)?).map_err(D::Error::custom)
}

pub fn serialize<S: Serializer>(length: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&friendly_duration_text(*length))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_spellings_parse_to_expected_lengths() {
        for (written_length, expected_length) in [
            ("0s", Duration::ZERO),
            ("10 s", Duration::from_secs(10)),
            ("140ms", Duration::from_millis(140)),
            ("1s 400ms", Duration::from_millis(1400)),
            ("15m", Duration::from_mins(15)),
            ("1 min", Duration::from_mins(1)),
            ("1h 30m", Duration::from_mins(90)),
            ("1h 30min", Duration::from_mins(90)),
            ("2 d", Duration::from_hours(48)),
            ("1week", Duration::from_hours(168)),
        ] {
            assert_eq!(
                parsed_friendly_duration(written_length).ok(),
                Some(expected_length),
                "{written_length}"
            );
        }
    }

    #[test]
    fn printed_lengths_read_back_unchanged() {
        for length in [
            Duration::from_millis(1400),
            Duration::from_mins(90),
            Duration::from_hours(48),
        ] {
            assert_eq!(
                parsed_friendly_duration(&friendly_duration_text(length)).ok(),
                Some(length)
            );
        }
    }
}
