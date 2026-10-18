use std::{error::Error, fmt, num::ParseFloatError, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum Spacing {
    Px(f32),
    Em(f32),
}

impl From<f32> for Spacing {
    fn from(pixels: f32) -> Self {
        Self::Px(pixels)
    }
}

impl Spacing {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> f32 {
        match self {
            Self::Px(px) => px,
            Self::Em(em) => em * em_px,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpacingParseError {
    MissingUnit(String),
    UnknownUnit(String),
    NotANumber(String, ParseFloatError),
}

impl fmt::Display for SpacingParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUnit(text) => {
                write!(
                    f,
                    "{text:?} needs a unit: write it as \"{text}px\" or in em"
                )
            }
            Self::UnknownUnit(unit) => write!(f, "unknown unit {unit:?}; use px or em"),
            Self::NotANumber(text, error) => write!(f, "{text:?} is not a number: {error}"),
        }
    }
}

impl Error for SpacingParseError {}

impl FromStr for Spacing {
    type Err = SpacingParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let trimmed = text.trim();
        let unit_start = trimmed
            .find(|character: char| character.is_ascii_alphabetic())
            .ok_or_else(|| SpacingParseError::MissingUnit(trimmed.to_owned()))?;
        let (number, unit) = trimmed.split_at(unit_start);
        let number = number.trim();
        let value = number
            .parse::<f32>()
            .map_err(|error| SpacingParseError::NotANumber(number.to_owned(), error))?;
        match unit {
            "px" => Ok(Self::Px(value)),
            "em" => Ok(Self::Em(value)),
            other => Err(SpacingParseError::UnknownUnit(other.to_owned())),
        }
    }
}

impl TryFrom<String> for Spacing {
    type Error = SpacingParseError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl fmt::Display for Spacing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Px(px) => write!(f, "{px}px"),
            Self::Em(em) => write!(f, "{em}em"),
        }
    }
}

impl From<Spacing> for String {
    fn from(spacing: Spacing) -> Self {
        spacing.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{Spacing, SpacingParseError};

    #[test]
    fn units_parse_and_print_back() {
        assert_eq!("10px".parse(), Ok(Spacing::Px(10.0)));
        assert_eq!(" 0.75 em ".parse(), Ok(Spacing::Em(0.75)));
        assert_eq!(Spacing::Px(15.5).to_string(), "15.5px");
        assert_eq!(Spacing::Em(2.0).to_string(), "2em");
    }

    #[test]
    fn bare_numbers_and_unknown_units_are_rejected() {
        assert_eq!(
            "10".parse::<Spacing>(),
            Err(SpacingParseError::MissingUnit("10".to_owned()))
        );
        assert_eq!(
            "3mm".parse::<Spacing>(),
            Err(SpacingParseError::UnknownUnit("mm".to_owned()))
        );
        assert!(matches!(
            "xpx".parse::<Spacing>(),
            Err(SpacingParseError::NotANumber(..))
        ));
    }

    #[test]
    fn em_resolves_against_the_base_size() {
        assert!((Spacing::Em(1.5).resolve(14.0) - 21.0).abs() < f32::EPSILON);
        assert!((Spacing::Px(9.0).resolve(14.0) - 9.0).abs() < f32::EPSILON);
    }
}
