use serde::{Deserialize, Deserializer};
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn bgra(self) -> [u8; 4] {
        [self.b, self.g, self.r, self.a]
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ColorParseError {
    MissingHash,
    InvalidLength(usize),
    InvalidHex(ParseIntError),
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHash => write!(f, "color must start with '#'"),
            Self::InvalidLength(n) => write!(f, "expected 6 hex digits, got {n}"),
            Self::InvalidHex(e) => write!(f, "invalid hex digit: {e}"),
        }
    }
}

impl std::error::Error for ColorParseError {}

impl FromStr for Color {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').ok_or(ColorParseError::MissingHash)?;
        if hex.len() != 6 {
            return Err(ColorParseError::InvalidLength(hex.len()));
        }
        let parse =
            |range| u8::from_str_radix(&hex[range], 16).map_err(ColorParseError::InvalidHex);
        Ok(Self::rgb(parse(0..2)?, parse(2..4)?, parse(4..6)?))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_rgb() {
        assert_eq!(
            "#285577".parse::<Color>().unwrap(),
            Color::rgb(0x28, 0x55, 0x77)
        );
        assert_eq!("#000000".parse::<Color>().unwrap(), Color::rgb(0, 0, 0));
        assert_eq!(
            "#ffffff".parse::<Color>().unwrap(),
            Color::rgb(255, 255, 255)
        );
    }

    #[test]
    fn rejects_missing_hash() {
        assert_eq!("285577".parse::<Color>(), Err(ColorParseError::MissingHash));
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(
            "#28557".parse::<Color>(),
            Err(ColorParseError::InvalidLength(5))
        );
        assert_eq!(
            "#2855778".parse::<Color>(),
            Err(ColorParseError::InvalidLength(7))
        );
    }

    #[test]
    fn rejects_non_hex() {
        assert!(matches!(
            "#zz5577".parse::<Color>(),
            Err(ColorParseError::InvalidHex(_))
        ));
    }
}
