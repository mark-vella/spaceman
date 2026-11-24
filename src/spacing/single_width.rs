//! `settings.single_window_max_width`: off, a width in points, or a share of the display.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use super::MIN_USABLE;

/// Largest `single_window_max_width` in points.
pub const MAX_WIDTH_LIMIT: i32 = 16_384;

/// How wide a lone tile (one tiled window, or Fullscreen Layout) is when its display is wider.
///
/// `config.toml` writes points as an integer (`0` is off) and a share as a string, `"80%"`; JSON
/// output uses the same forms.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SingleWidth {
    #[default]
    Off,
    Points(WidthPoints),
    Percent(WidthPercent),
}

/// A width from [`MIN_USABLE`]`.0` to [`MAX_WIDTH_LIMIT`] points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WidthPoints(u16);

/// A whole percentage from 1 to 100 of the display's usable width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WidthPercent(u8);

impl WidthPoints {
    #[must_use]
    pub const fn new(points: i64) -> Option<Self> {
        if points >= MIN_USABLE.0 as i64 && points <= MAX_WIDTH_LIMIT as i64 {
            Some(Self(points as u16))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> i32 {
        self.0 as i32
    }
}

impl WidthPercent {
    #[must_use]
    pub const fn new(percent: i64) -> Option<Self> {
        if percent >= 1 && percent <= 100 {
            Some(Self(percent as u8))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> i32 {
        self.0 as i32
    }
}

impl SingleWidth {
    /// The key under `[settings]`.
    pub const KEY: &'static str = "single_window_max_width";

    /// An integer from `config.toml` or the CLI: `0` is off, else a width in points.
    pub fn from_points(points: i64) -> Result<Self, String> {
        if points == 0 {
            return Ok(Self::Off);
        }
        WidthPoints::new(points)
            .map(Self::Points)
            .ok_or_else(|| invalid(points))
    }

    /// A string from `config.toml`: a whole percentage such as `"80%"`.
    pub fn from_text(raw: &str) -> Result<Self, String> {
        raw.strip_suffix('%')
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse::<i64>().ok())
            .and_then(WidthPercent::new)
            .map(Self::Percent)
            .ok_or_else(|| invalid(format!("{raw:?}")))
    }

    /// A CLI argument: `80%`, or an integer as [`SingleWidth::from_points`] takes it.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.parse::<i64>() {
            Ok(points) => Self::from_points(points),
            Err(_) => Self::from_text(raw),
        }
    }

    /// The value `config.toml` holds for the key, an integer or a string.
    pub fn from_toml(value: &toml::Value) -> Result<Self, String> {
        match value {
            toml::Value::Integer(points) => Self::from_points(*points),
            toml::Value::String(text) => Self::from_text(text),
            other => Err(invalid(other)),
        }
    }

    #[must_use]
    pub const fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }

    /// The lone tile's width in a usable rect `usable_width` wide, or `None` when it fills it.
    /// A share never drops below [`MIN_USABLE`]`.0` unless the usable rect is narrower still.
    #[must_use]
    pub fn width_in(self, usable_width: i32) -> Option<i32> {
        let width = match self {
            Self::Off => return None,
            Self::Points(points) => points.get(),
            Self::Percent(percent) => {
                let share = i64::from(usable_width) * i64::from(percent.get()) / 100;
                (share as i32).max(MIN_USABLE.0)
            }
        };
        (width < usable_width).then_some(width)
    }
}

fn invalid(got: impl fmt::Display) -> String {
    format!(
        "settings.{} must be 0 (off), between {} and {MAX_WIDTH_LIMIT} px, or a percentage \
         from \"1%\" to \"100%\", got {got}",
        SingleWidth::KEY,
        MIN_USABLE.0
    )
}

/// As `config get` prints it: `0`, `1680` or `80%`.
impl fmt::Display for SingleWidth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => formatter.write_str("0"),
            Self::Points(points) => write!(formatter, "{}", points.get()),
            Self::Percent(percent) => write!(formatter, "{}%", percent.get()),
        }
    }
}

impl Serialize for SingleWidth {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Off => serializer.serialize_i32(0),
            Self::Points(points) => serializer.serialize_i32(points.get()),
            Self::Percent(_) => serializer.collect_str(self),
        }
    }
}

impl<'de> Deserialize<'de> for SingleWidth {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_toml(&toml::Value::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}
