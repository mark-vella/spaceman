//! Display discovery: each connected screen's stable ID, name and visible frame.

use std::{collections::HashMap, ptr::NonNull};

use objc2::{MainThreadMarker, rc::autoreleasepool};
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CFRetained, CFUUID};
use objc2_foundation::{NSNumber, ns_string};

use crate::{DisplaySelector, Rect};

use super::SnapshotError;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> *const CFUUID;
}

pub(super) fn main_output() -> Result<Rect, SnapshotError> {
    Ok(outputs()?.main().frame)
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct DisplayOutput {
    pub id: String,
    pub name: String,
    pub frame: Rect,
    pub is_main: bool,
}

/// Maximum distance outside a display's visible tiling frame (in points) that a
/// synthetic `DisplayOutputs::new` still attributes to that display's reserved
/// menu-bar or Dock strip when full physical screen bounds were not provided.
const SYNTHETIC_STRIP_MARGIN: i64 = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayOutputs {
    displays: Vec<DisplayOutput>,
    /// Full physical screen bounds (including menu bar, notch, and Dock), parallel
    /// to `displays`, when discovered from macOS `NSScreen` or supplied via
    /// `with_bounds`. `None` when constructed from usable frames alone.
    bounds: Option<Vec<Rect>>,
    main_index: usize,
}

impl DisplayOutputs {
    pub fn new(displays: Vec<DisplayOutput>) -> Result<Self, SnapshotError> {
        let Some(main_index) = displays.iter().position(|display| display.is_main) else {
            return Err(SnapshotError::MainScreenUnavailable);
        };
        Ok(Self {
            displays,
            bounds: None,
            main_index,
        })
    }

    pub fn with_bounds(
        displays: Vec<DisplayOutput>,
        bounds: Vec<Rect>,
    ) -> Result<Self, SnapshotError> {
        let mut outputs = Self::new(displays)?;
        if bounds.len() == outputs.displays.len() {
            outputs.bounds = Some(bounds);
        }
        Ok(outputs)
    }

    #[must_use]
    pub fn displays(&self) -> &[DisplayOutput] {
        &self.displays
    }

    #[must_use]
    pub fn main(&self) -> &DisplayOutput {
        &self.displays[self.main_index]
    }

    #[must_use]
    pub fn secondary(&self) -> Option<&DisplayOutput> {
        self.displays.iter().find(|display| !display.is_main)
    }

    #[must_use]
    pub fn resolve_exact(&self, selector: &DisplaySelector) -> Option<&DisplayOutput> {
        match selector {
            DisplaySelector::Main => Some(self.main()),
            DisplaySelector::Secondary => self.secondary(),
            DisplaySelector::Preferred(preferred) => self
                .displays
                .iter()
                .find(|display| display.id == *preferred || display.name == *preferred),
        }
    }

    /// Whether `selector` picks `display` itself, with no fallback to main: how a
    /// `[[displays]]` entry finds the displays it applies to.
    #[must_use]
    pub fn selects(&self, selector: &DisplaySelector, display: &DisplayOutput) -> bool {
        self.resolve_exact(selector)
            .is_some_and(|picked| picked.id == display.id)
    }

    #[must_use]
    pub fn resolve(&self, selector: &DisplaySelector) -> &DisplayOutput {
        self.resolve_exact(selector).unwrap_or_else(|| self.main())
    }

    /// The canonical `config.toml` selector for `display`: `main`, `secondary` (the first
    /// non-main display), or the display's own ID for every display past those two.
    #[must_use]
    pub fn selector_for(&self, display: &DisplayOutput) -> DisplaySelector {
        if display.is_main {
            DisplaySelector::Main
        } else if self
            .secondary()
            .is_some_and(|secondary| secondary.id == display.id)
        {
            DisplaySelector::Secondary
        } else {
            DisplaySelector::Preferred(display.id.clone())
        }
    }

    /// Every display left to right, then top to bottom, ties broken by ID.
    #[must_use]
    pub fn ordered(&self) -> Vec<&DisplayOutput> {
        let mut displays: Vec<_> = self.displays.iter().collect();
        displays.sort_by(|left, right| {
            (left.frame.x, left.frame.y, left.id.as_str()).cmp(&(
                right.frame.x,
                right.frame.y,
                right.id.as_str(),
            ))
        });
        displays
    }

    pub fn frames(&self) -> impl Iterator<Item = Rect> + '_ {
        self.displays.iter().map(|display| display.frame)
    }

    #[must_use]
    pub fn union(&self) -> Rect {
        self.frames().fold(self.main().frame, union_rect)
    }

    /// Physical display containing `point`, including its menu-bar and Dock strips.
    ///
    /// Uses full `NSScreen::frame()` bounds when available (with a 1-point tolerance
    /// for rounded screen-edge pointer coordinates). When only usable frames were
    /// provided (synthetic fixtures), falls back to the nearest display whose frame
    /// lies within a reserved-strip margin.
    #[must_use]
    pub fn display_at(&self, point: (i32, i32)) -> Option<&DisplayOutput> {
        let (x, y) = point;
        if let Some(bounds) = &self.bounds {
            if let Some(index) = bounds.iter().position(|rect| rect.contains(x, y)) {
                return Some(&self.displays[index]);
            }
            return self.nearest_within(bounds, x, y, 1);
        }
        if let Some(display) = self.displays.iter().find(|d| d.frame.contains(x, y)) {
            return Some(display);
        }
        let frames: Vec<Rect> = self.displays.iter().map(|d| d.frame).collect();
        self.nearest_within(&frames, x, y, SYNTHETIC_STRIP_MARGIN)
    }

    /// Physical display for `point`, matching menu-bar and Dock strips and falling
    /// back to the nearest display rather than jumping straight to main.
    #[must_use]
    pub fn display_for_point(&self, point: (i32, i32)) -> &DisplayOutput {
        if let Some(display) = self.display_at(point) {
            return display;
        }
        let (x, y) = point;
        let rects = self.bounds.as_deref();
        self.displays
            .iter()
            .enumerate()
            .min_by_key(|(index, display)| {
                let rect = rects.map_or(display.frame, |bounds| bounds[*index]);
                let (dx, dy) = rect_axis_distance(rect, x, y);
                (dx * dx + dy * dy, !display.is_main)
            })
            .map_or_else(|| self.main(), |(_, display)| display)
    }

    fn nearest_within(
        &self,
        rects: &[Rect],
        x: i32,
        y: i32,
        max_margin: i64,
    ) -> Option<&DisplayOutput> {
        self.displays
            .iter()
            .enumerate()
            .filter_map(|(index, display)| {
                let (dx, dy) = rect_axis_distance(rects[index], x, y);
                (dx <= max_margin && dy <= max_margin).then_some((
                    dx * dx + dy * dy,
                    !display.is_main,
                    display,
                ))
            })
            .min_by_key(|(distance, not_main, _)| (*distance, *not_main))
            .map(|(_, _, display)| display)
    }
}

pub fn outputs() -> Result<DisplayOutputs, SnapshotError> {
    autoreleasepool(|_| {
        let marker = MainThreadMarker::new().ok_or(SnapshotError::MainThreadRequired)?;
        let screens = NSScreen::screens(marker);
        let primary_height = (0..screens.count())
            .map(|index| screens.objectAtIndex(index).frame())
            .find(|frame| frame.origin.x == 0.0 && frame.origin.y == 0.0)
            .map(|frame| frame.size.height)
            .ok_or(SnapshotError::MainScreenUnavailable)?;
        let mut displays = Vec::with_capacity(screens.count());
        let mut bounds = Vec::with_capacity(screens.count());
        for index in 0..screens.count() {
            let screen = screens.objectAtIndex(index);
            let frame = screen.frame();
            let display_id =
                screen_display_id(&screen).ok_or(SnapshotError::MainScreenUnavailable)?;
            displays.push(DisplayOutput {
                id: display_uuid(display_id).unwrap_or_else(|| display_id.to_string()),
                name: screen.localizedName().to_string(),
                frame: screen_output(&screen, primary_height),
                is_main: frame.origin.x == 0.0 && frame.origin.y == 0.0,
            });
            bounds.push(screen_rect(frame, primary_height));
        }
        DisplayOutputs::with_bounds(displays, bounds)
    })
}

/// Each connected display's Core Graphics ID, keyed by the ID `outputs` gives it. Main thread
/// only; empty elsewhere.
#[must_use]
pub fn display_ids() -> HashMap<String, u32> {
    autoreleasepool(|_| {
        let Some(marker) = MainThreadMarker::new() else {
            return HashMap::new();
        };
        let screens = NSScreen::screens(marker);
        (0..screens.count())
            .filter_map(|index| screen_display_id(&screens.objectAtIndex(index)))
            .map(|display_id| {
                let id = display_uuid(display_id).unwrap_or_else(|| display_id.to_string());
                (id, display_id)
            })
            .collect()
    })
}

pub(super) fn screen_display_id(screen: &NSScreen) -> Option<u32> {
    let description = screen.deviceDescription();
    let number = description.objectForKey(ns_string!("NSScreenNumber"))?;
    number
        .downcast_ref::<NSNumber>()
        .map(NSNumber::unsignedIntValue)
}

fn display_uuid(display_id: u32) -> Option<String> {
    // SAFETY: `CGDisplayCreateUUIDFromDisplayID` returns null or a +1 retained `CFUUIDRef`.
    let raw = unsafe { CGDisplayCreateUUIDFromDisplayID(display_id) };
    // SAFETY: `raw` is non-null and +1 retained under the Core Foundation Create rule.
    let uuid = unsafe { CFRetained::from_raw(NonNull::new(raw.cast_mut())?) };
    CFUUID::new_string(None, Some(&uuid)).map(|uuid| uuid.to_string())
}

fn screen_output(screen: &NSScreen, primary_height: f64) -> Rect {
    screen_rect(screen.visibleFrame(), primary_height)
}

fn screen_rect(rect: objc2_foundation::NSRect, primary_height: f64) -> Rect {
    Rect {
        x: rect.origin.x.round() as i32,
        y: (primary_height - rect.origin.y - rect.size.height).round() as i32,
        width: rect.size.width.round() as i32,
        height: rect.size.height.round() as i32,
    }
}

/// Non-negative distance along each axis from `(x, y)` to the closed pixel
/// region covered by `rect`.
fn rect_axis_distance(rect: Rect, x: i32, y: i32) -> (i64, i64) {
    let min_x = i64::from(rect.x);
    let max_x = min_x + i64::from(rect.width).saturating_sub(1).max(0);
    let min_y = i64::from(rect.y);
    let max_y = min_y + i64::from(rect.height).saturating_sub(1).max(0);
    let dx = if i64::from(x) < min_x {
        min_x - i64::from(x)
    } else if i64::from(x) > max_x {
        i64::from(x) - max_x
    } else {
        0
    };
    let dy = if i64::from(y) < min_y {
        min_y - i64::from(y)
    } else if i64::from(y) > max_y {
        i64::from(y) - max_y
    } else {
        0
    };
    (dx, dy)
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    Rect {
        x,
        y,
        width: (left.x + left.width).max(right.x + right.width) - x,
        height: (left.y + left.height).max(right.y + right.height) - y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: &str, name: &str, x: i32, is_main: bool) -> DisplayOutput {
        DisplayOutput {
            id: id.into(),
            name: name.into(),
            frame: Rect {
                x,
                y: 0,
                width: 100,
                height: 100,
            },
            is_main,
        }
    }

    fn three_displays() -> DisplayOutputs {
        DisplayOutputs::new(vec![
            display("built-in", "Built-in Retina Display", 0, true),
            display("studio-a", "Studio Display", 100, false),
            display("studio-b", "Studio Display", -100, false),
        ])
        .unwrap()
    }

    #[test]
    fn selector_for_names_main_secondary_and_every_further_display_by_id() {
        let outputs = three_displays();
        let selectors: Vec<String> = outputs
            .displays()
            .iter()
            .map(|display| outputs.selector_for(display).as_str().to_owned())
            .collect();
        assert_eq!(selectors, ["main", "secondary", "studio-b"]);
    }

    #[test]
    fn ordered_lists_displays_left_to_right() {
        let outputs = three_displays();
        let ids: Vec<&str> = outputs
            .ordered()
            .iter()
            .map(|display| display.id.as_str())
            .collect();
        assert_eq!(ids, ["studio-b", "built-in", "studio-a"]);
    }

    #[test]
    fn display_at_matches_reserved_strips_with_bounds_and_synthetic_frames() {
        let visible = Rect {
            x: 0,
            y: 25,
            width: 1000,
            height: 750,
        };
        let second_visible = Rect { x: 1000, ..visible };
        let synthetic = DisplayOutputs::new(vec![
            DisplayOutput {
                id: "main".into(),
                name: "Main".into(),
                frame: visible,
                is_main: true,
            },
            DisplayOutput {
                id: "second".into(),
                name: "Second".into(),
                frame: second_visible,
                is_main: false,
            },
        ])
        .unwrap();
        assert_eq!(
            synthetic.display_at((500, 10)).map(|d| d.id.as_str()),
            Some("main")
        );
        assert_eq!(
            synthetic.display_at((500, 790)).map(|d| d.id.as_str()),
            Some("main")
        );
        assert_eq!(
            synthetic.display_at((1500, 10)).map(|d| d.id.as_str()),
            Some("second")
        );
        assert_eq!(
            synthetic.display_at((1500, 790)).map(|d| d.id.as_str()),
            Some("second")
        );
        assert_eq!(synthetic.display_at((5000, 5000)), None);
        assert_eq!(synthetic.display_for_point((1500, 10)).id, "second");

        // Full physical bounds also resolve asymmetric Docks unambiguously (e.g. a
        // bottom Dock on main adjacent to a full-height secondary display).
        let with_bounds = DisplayOutputs::with_bounds(
            vec![
                DisplayOutput {
                    id: "main".into(),
                    name: "Main".into(),
                    frame: Rect {
                        x: 0,
                        y: 25,
                        width: 1000,
                        height: 700,
                    },
                    is_main: true,
                },
                DisplayOutput {
                    id: "second".into(),
                    name: "Second".into(),
                    frame: Rect {
                        x: 1000,
                        y: 25,
                        width: 1000,
                        height: 775,
                    },
                    is_main: false,
                },
            ],
            vec![
                Rect {
                    x: 0,
                    y: 0,
                    width: 1000,
                    height: 800,
                },
                Rect {
                    x: 1000,
                    y: 0,
                    width: 1000,
                    height: 800,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            with_bounds.display_at((990, 750)).map(|d| d.id.as_str()),
            Some("main")
        );
        assert_eq!(
            with_bounds.display_at((1000, 10)).map(|d| d.id.as_str()),
            Some("second")
        );
        assert_eq!(
            with_bounds.display_at((2000, 800)).map(|d| d.id.as_str()),
            Some("second")
        );
        assert_eq!(with_bounds.display_at((2010, 800)), None);
    }
}
