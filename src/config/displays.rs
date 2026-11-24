//! `[[displays]]`: settings that differ per display. Each entry names displays with the selector
//! `[[workspaces]]` uses and overrides the global value on them; for each display the first
//! entry, in file order, whose selector picks it wins.

use serde::Deserialize;

use crate::{
    DisplaySelector,
    spacing::{Spacing, SpacingKey},
};

use super::Config;

/// One `[[displays]]` entry, its values checked like their `[settings]` counterparts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayOverride {
    pub monitor: DisplaySelector,
    /// Replaces `settings.single_window_max_width` on this display; `0` is off.
    pub single_window_max_width: i32,
}

impl DisplayOverride {
    /// `spacing` on a display this entry applies to.
    #[must_use]
    pub const fn apply(&self, spacing: Spacing) -> Spacing {
        Spacing {
            single_window_max_width: self.single_window_max_width,
            ..spacing
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDisplay {
    monitor: String,
    single_window_max_width: Option<i64>,
}

pub(super) fn parse(raw: Vec<RawDisplay>) -> Result<Vec<DisplayOverride>, String> {
    let mut overrides: Vec<DisplayOverride> = Vec::with_capacity(raw.len());
    for entry in raw {
        let monitor = entry.monitor;
        if monitor.trim().is_empty() {
            return Err("display monitor cannot be empty".into());
        }
        if overrides
            .iter()
            .any(|existing| existing.monitor.as_str() == monitor)
        {
            return Err(format!("display {monitor:?} is configured more than once"));
        }
        let Some(width) = entry.single_window_max_width else {
            return Err(format!(
                "display {monitor:?} overrides nothing; set single_window_max_width"
            ));
        };
        let single_window_max_width =
            SpacingKey::SingleWindowMaxWidth.check_at(&format!("display {monitor:?} "), width)?;
        overrides.push(DisplayOverride {
            monitor: DisplaySelector::parse(&monitor),
            single_window_max_width,
        });
    }
    Ok(overrides)
}

impl Config {
    /// Every `[[displays]]` entry, in file order.
    #[must_use]
    pub fn display_overrides(&self) -> &[DisplayOverride] {
        &self.displays
    }

    /// The entry, and its position among `[[displays]]`, that applies to the display `selects`
    /// answers for: the first whose `monitor` it picks.
    pub fn display_override(
        &self,
        selects: impl Fn(&DisplaySelector) -> bool,
    ) -> Option<(usize, &DisplayOverride)> {
        self.displays
            .iter()
            .enumerate()
            .find(|(_, entry)| selects(&entry.monitor))
    }

    /// The spacing on one display: [`Config::spacing`] with that display's `[[displays]]` entry
    /// applied. `selects` says whether a selector picks the display, as
    /// `macos::DisplayOutputs::selects` does.
    pub fn spacing_on(&self, selects: impl Fn(&DisplaySelector) -> bool) -> Spacing {
        let spacing = self.spacing();
        self.display_override(selects)
            .map_or(spacing, |(_, entry)| entry.apply(spacing))
    }
}

#[cfg(test)]
mod tests;
