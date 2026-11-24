//! The displays Settings › Workspaces offers: every connected screen, its friendly label, and
//! the exact `config.toml` selector the engine resolves (the same one Option+Shift+Tab writes).

use spaceman::Rect;
use spaceman::macos::{DisplayOutput, DisplayOutputs};

/// The built-in selector for the display macOS marks as main.
pub(crate) const MAIN: &str = "main";
/// The built-in selector for the first non-main display.
pub(crate) const SECONDARY: &str = "secondary";

/// One connected display, read from `spaceman::macos::outputs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConnectedDisplay {
    /// `main`, `secondary`, or the display's UUID.
    pub selector: String,
    /// The display's own ID, as `DisplayOutput::id` gives it.
    pub id: String,
    /// The display's own name, as macOS reports it ("Built-in Retina Display").
    pub name: String,
    pub frame: Rect,
    pub is_main: bool,
    pub is_secondary: bool,
}

impl ConnectedDisplay {
    fn is_built_in(&self) -> bool {
        self.is_main || self.is_secondary
    }
}

/// Every connected display with the selector Settings writes for it.
pub(crate) fn connected(outputs: &DisplayOutputs) -> Vec<ConnectedDisplay> {
    outputs
        .displays()
        .iter()
        .map(|display| {
            let selector = outputs.selector_for(display).as_str().to_owned();
            ConnectedDisplay {
                is_secondary: selector == SECONDARY,
                selector,
                id: display.id.clone(),
                name: display.name.clone(),
                frame: display.frame,
                is_main: display.is_main,
            }
        })
        .collect()
}

/// `displays` as the engine sees them, so a selector picks the same display here as there.
pub(crate) fn outputs(displays: &[ConnectedDisplay]) -> Option<DisplayOutputs> {
    DisplayOutputs::new(
        displays
            .iter()
            .map(|display| DisplayOutput {
                id: display.id.clone(),
                name: display.name.clone(),
                frame: display.frame,
                is_main: display.is_main,
            })
            .collect(),
    )
    .ok()
}

/// The `monitor` Settings writes in a new `[[displays]]` entry: the display's own name while no
/// other connected display shares it, else its UUID. Unlike `main` or `secondary`, both stay
/// with the screen when another display becomes main.
pub(crate) fn monitor_for(outputs: &DisplayOutputs, display: &DisplayOutput) -> String {
    let shared = outputs
        .displays()
        .iter()
        .any(|other| other.id != display.id && other.name == display.name);
    if display.name.trim().is_empty() || shared {
        display.id.clone()
    } else {
        display.name.clone()
    }
}

/// One segment of a workspace's display picker and one section of the pane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Choice {
    /// The selector written to `config.toml` when this choice is picked.
    pub selector: String,
    /// The picker segment's label, kept short so segments never overflow.
    pub label: String,
    /// The section heading above the workspaces assigned to it.
    pub title: String,
    /// The heading's trailing caption.
    pub caption: String,
    /// The display's UUID, so a saved `monitor` naming it resolves here.
    pub id: Option<String>,
    /// The display's name, so a saved `monitor` naming it resolves here.
    pub name: Option<String>,
    /// Whether a display is plugged in for this choice.
    pub connected: bool,
}

impl Choice {
    /// Whether the `monitor` value `selector` names this display, exactly as
    /// `DisplayOutputs::resolve_exact` does: by built-in selector, UUID, or name.
    fn matches(&self, selector: &str) -> bool {
        self.selector == selector
            || self.id.as_deref() == Some(selector)
            || self.name.as_deref() == Some(selector)
    }
}

/// The choice `selector` (a `config.toml` `monitor` value) resolves to, if any display here
/// answers to it.
pub(crate) fn index_of(choices: &[Choice], selector: &str) -> Option<usize> {
    choices.iter().position(|choice| choice.matches(selector))
}

/// Main, then Secondary, then every further display left to right. Main and Secondary are always
/// offered, even when nothing is plugged into them, because workspaces fall back to main.
pub(crate) fn choices(displays: &[ConnectedDisplay]) -> Vec<Choice> {
    let mut ordered: Vec<&ConnectedDisplay> = displays.iter().collect();
    ordered.sort_by(|left, right| {
        rank(left).cmp(&rank(right)).then_with(|| {
            (left.frame.x, left.frame.y, left.selector.as_str()).cmp(&(
                right.frame.x,
                right.frame.y,
                right.selector.as_str(),
            ))
        })
    });
    let main = ordered.iter().copied().find(|display| display.is_main);
    let secondary = ordered.iter().copied().find(|display| display.is_secondary);
    let mut choices = vec![
        built_in(MAIN, "Main", "Main display", main),
        built_in(SECONDARY, "Secondary", "Secondary display", secondary),
    ];
    choices[0].caption = match &main {
        Some(display) if !display.name.is_empty() => {
            format!("{} · Default for 1–5", display.name)
        }
        _ => "Default for 1–5".to_owned(),
    };
    choices[1].caption = match &secondary {
        Some(display) if !display.name.is_empty() => {
            format!("{} · Default for 6–10", display.name)
        }
        Some(_) => "Default for 6–10".to_owned(),
        None => "Default for 6–10 · main when unplugged".to_owned(),
    };
    let extras: Vec<&ConnectedDisplay> = ordered
        .iter()
        .copied()
        .filter(|display| !display.is_built_in())
        .collect();
    for display in &extras {
        let title = extra_name(choices.len() + 1, display, &ordered);
        let label = extra_label(choices.len() + 1, &title, ordered.len());
        choices.push(Choice {
            selector: display.selector.clone(),
            title,
            label,
            caption: format!("Display {}", choices.len() + 1),
            id: Some(display.id.clone()),
            name: Some(display.name.clone()),
            connected: true,
        });
    }
    choices
}

fn rank(display: &ConnectedDisplay) -> u8 {
    if display.is_main {
        0
    } else if display.is_secondary {
        1
    } else {
        2
    }
}

fn built_in(
    selector: &str,
    label: &str,
    title: &str,
    display: Option<&ConnectedDisplay>,
) -> Choice {
    Choice {
        selector: selector.to_owned(),
        label: label.to_owned(),
        title: title.to_owned(),
        caption: String::new(),
        id: display.map(|display| display.id.clone()),
        name: display.map(|display| display.name.clone()),
        connected: display.is_some(),
    }
}

/// The heading for a display past Secondary: its own name, "Studio Display 2" when two monitors
/// share one, and its position ("Display 3") for a long or missing name.
fn extra_name(
    position: usize,
    display: &ConnectedDisplay,
    displays: &[&ConnectedDisplay],
) -> String {
    let name = display.name.trim();
    let same: Vec<&&ConnectedDisplay> = displays
        .iter()
        .filter(|other| other.name.trim() == name)
        .collect();
    if same.len() > 1 && !name.is_empty() {
        let ordinal = same
            .iter()
            .position(|other| other.selector == display.selector)
            .unwrap_or_default()
            + 1;
        return format!("{name} {ordinal}");
    }
    if name.is_empty() || name.chars().count() > 16 {
        format!("Display {position}")
    } else {
        name.to_owned()
    }
}

/// The picker segment for a display past Secondary. With four or more displays even short names
/// crowd the segmented control, so those get their position and the heading keeps the name.
fn extra_label(position: usize, name: &str, displays: usize) -> String {
    if displays > 3 {
        format!("Display {position}")
    } else {
        name.to_owned()
    }
}

/// Two displays, as a fresh Mac reports them: the built-in main and one Studio Display.
pub(crate) fn fixture_pair() -> Vec<ConnectedDisplay> {
    vec![
        ConnectedDisplay {
            selector: MAIN.to_owned(),
            id: "37D8832A-2D66-02CA-B9F7-8F30A301B230".to_owned(),
            name: "Built-in Retina Display".to_owned(),
            frame: Rect {
                x: 0,
                y: 0,
                width: 1728,
                height: 1117,
            },
            is_main: true,
            is_secondary: false,
        },
        ConnectedDisplay {
            selector: SECONDARY.to_owned(),
            id: "4A1B2C3D-5E6F-7081-92A3-B4C5D6E7F801".to_owned(),
            name: "Studio Display".to_owned(),
            frame: Rect {
                x: 1728,
                y: 0,
                width: 2560,
                height: 1440,
            },
            is_main: false,
            is_secondary: true,
        },
    ]
}

/// A MacBook's built-in main display and a 34-inch ultrawide to its right.
pub(crate) fn fixture_laptop_and_ultrawide() -> Vec<ConnectedDisplay> {
    let mut displays = fixture_pair();
    displays[1].id = "6E1F2A3B-4C5D-4E6F-8091-A2B3C4D5E6F7".to_owned();
    displays[1].name = "M34WQ".to_owned();
    displays[1].frame = Rect {
        x: 1728,
        y: 0,
        width: 3440,
        height: 1440,
    };
    displays
}

/// The third display's UUID in the Playground's three-display scene.
pub(crate) const THIRD_FIXTURE_ID: &str = "8F14E45F-EA61-4B2A-9C3D-1B2C3D4E5F60";

/// The fourth display's UUID in the Playground's four-display scene.
pub(crate) const FOURTH_FIXTURE_ID: &str = "A1B2C3D4-E5F6-7081-92A3-B4C5D6E7F809";

/// Three displays: the built-in main and two identically named external monitors.
pub(crate) fn fixture_three() -> Vec<ConnectedDisplay> {
    let pair = fixture_pair();
    vec![
        pair[0].clone(),
        pair[1].clone(),
        ConnectedDisplay {
            selector: THIRD_FIXTURE_ID.to_owned(),
            id: THIRD_FIXTURE_ID.to_owned(),
            name: "Studio Display".to_owned(),
            frame: Rect {
                x: -2560,
                y: 0,
                width: 2560,
                height: 1440,
            },
            is_main: false,
            is_secondary: false,
        },
    ]
}

/// Four displays: the three-display pair plus a differently named external on the right.
pub(crate) fn fixture_quad() -> Vec<ConnectedDisplay> {
    let mut displays = fixture_three();
    displays.push(ConnectedDisplay {
        selector: FOURTH_FIXTURE_ID.to_owned(),
        id: FOURTH_FIXTURE_ID.to_owned(),
        name: "LG UltraFine 5K".to_owned(),
        frame: Rect {
            x: 4288,
            y: 0,
            width: 2560,
            height: 1440,
        },
        is_main: false,
        is_secondary: false,
    });
    displays
}
