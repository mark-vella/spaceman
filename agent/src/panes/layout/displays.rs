//! Which display Settings › Layout's single-window width applies to: "All displays", the
//! `[settings]` width, or one connected display with its own `[[displays]]` entry.

use rune::{El, Tx};
use rune_kit::SegmentedControl;
use spaceman::DisplaySelector;

use crate::{
    Ui,
    controls::{self, ButtonKind, Paint},
    panes::workspaces::{self, ConnectedDisplay},
};

/// The single-window width on one connected display.
pub(super) struct DisplayWidth<'a> {
    pub display: &'a ConnectedDisplay,
    pub label: String,
    /// What a lone window gets there: the display's own width, else the one for all displays.
    pub width: i32,
    /// Whether a `[[displays]]` entry gives the display its own width.
    pub own: bool,
}

/// The display the width control edits, while one is picked and still connected.
pub(super) fn picked(ui: &Ui) -> Option<DisplayWidth<'_>> {
    let id = ui.layout_display.as_deref()?;
    let index = ui.displays.iter().position(|display| display.id == id)?;
    let resolved = workspaces::outputs(&ui.displays)
        .zip(ui.doc.config())
        .and_then(|(outputs, config)| {
            let output = outputs.displays().get(index)?;
            let selects = |selector: &DisplaySelector| outputs.selects(selector, output);
            Some((
                config.spacing_on(selects).single_window_max_width,
                config.display_override(selects).is_some(),
            ))
        });
    let (width, own) = resolved.unwrap_or((ui.doc.spacing().single_window_max_width, false));
    Some(DisplayWidth {
        display: &ui.displays[index],
        label: label(&ui.displays, index),
        width,
        own,
    })
}

/// A display's name, numbered when another display shares it, or its position without one.
fn label(displays: &[ConnectedDisplay], index: usize) -> String {
    let name = displays[index].name.trim();
    if name.is_empty() {
        return format!("Display {}", index + 1);
    }
    let same: Vec<usize> = (0..displays.len())
        .filter(|other| displays[*other].name.trim() == name)
        .collect();
    match same.iter().position(|other| *other == index) {
        Some(ordinal) if same.len() > 1 => format!("{name} {}", ordinal + 1),
        _ => name.to_owned(),
    }
}

/// "All displays", then each connected display; none while no display is known.
pub(super) fn picker_row(p: Paint, ui: &Ui) -> Option<El> {
    if ui.displays.is_empty() {
        return None;
    }
    let mut labels = vec!["All displays".to_owned()];
    labels.extend((0..ui.displays.len()).map(|index| label(&ui.displays, index)));
    let ids: Vec<Option<String>> = std::iter::once(None)
        .chain(ui.displays.iter().map(|display| Some(display.id.clone())))
        .collect();
    let current = picked(ui).map(|picked| picked.display.id.clone());
    let selected = ids.iter().position(|id| *id == current).unwrap_or(0);
    let control = SegmentedControl::new(&labels, Some(selected))
        .fit()
        .on_pick(move |ui: &mut Ui, index, _: &mut Tx| {
            if let Some(id) = ids.get(index) {
                ui.layout_display.clone_from(id);
            }
        })
        .build(p.k)
        .key("display-picker");
    Some(controls::setting_row(
        p, "display", "Display", None, control,
    ))
}

/// The width row's description for the picked display.
pub(super) fn width_caption(picked: &DisplayWidth<'_>) -> String {
    if picked.own {
        format!(
            "A window alone on {}, and Fullscreen Layout there, is centered at this width.",
            picked.label
        )
    } else {
        format!(
            "{} follows All displays. Pick a width to give it its own.",
            picked.label
        )
    }
}

/// While the picked display has its own width: the way back to following All displays.
pub(super) fn follow_row(p: Paint, picked: &DisplayWidth<'_>) -> Option<El> {
    if !picked.own {
        return None;
    }
    let id = picked.display.id.clone();
    Some(controls::setting_row(
        p,
        "follow",
        "Own width",
        Some((
            format!("Saved for {} in config.toml.", picked.label),
            p.muted(),
        )),
        controls::button(
            p,
            "follow-button",
            "Follow All displays",
            ButtonKind::Secondary,
            move |ui: &mut Ui, _| ui.set_display_width(&id, None),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pane, playground};

    const ULTRAWIDE_ID: &str = "6E1F2A3B-4C5D-4E6F-8091-A2B3C4D5E6F7";

    fn layout_with_two_displays() -> Ui {
        let mut ui = playground::blank_fixture();
        playground::reset_fixture(&mut ui, Pane::Layout);
        ui.displays = workspaces::fixture_laptop_and_ultrawide();
        ui.set_spacing(spaceman::spacing::Spacing {
            single_window_max_width: 1680,
            ..spaceman::spacing::Spacing::NONE
        });
        ui
    }

    #[test]
    fn a_picked_display_follows_all_displays_until_it_gets_its_own_width() {
        let mut ui = layout_with_two_displays();
        assert!(picked(&ui).is_none());

        ui.layout_display = Some(ULTRAWIDE_ID.to_owned());
        let width = picked(&ui).unwrap();
        assert_eq!(
            (width.label.as_str(), width.width, width.own),
            ("M34WQ", 1680, false)
        );

        ui.set_display_width(ULTRAWIDE_ID, Some(1920));
        let width = picked(&ui).unwrap();
        assert_eq!((width.width, width.own), (1920, true));
        assert!(
            ui.doc
                .source()
                .contains("[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 1920\n"),
            "{}",
            ui.doc.source()
        );
        assert_eq!(ui.doc.spacing().single_window_max_width, 1680);

        ui.set_display_width(ULTRAWIDE_ID, None);
        let width = picked(&ui).unwrap();
        assert_eq!((width.width, width.own), (1680, false));
        assert!(
            !ui.doc.source().contains("[[displays]]"),
            "{}",
            ui.doc.source()
        );
    }

    #[test]
    fn an_override_of_off_fills_the_picked_display() {
        let mut ui = layout_with_two_displays();
        let built_in = ui.displays[0].id.clone();
        ui.layout_display = Some(built_in.clone());

        ui.set_display_width(&built_in, Some(0));

        let width = picked(&ui).unwrap();
        assert_eq!(
            (width.label.as_str(), width.width, width.own),
            ("Built-in Retina Display", 0, true)
        );
    }

    #[test]
    fn an_unplugged_display_keeps_its_entry_and_drops_out_of_the_picker() {
        let mut ui = layout_with_two_displays();
        ui.set_display_width(ULTRAWIDE_ID, Some(2670));
        ui.layout_display = Some(ULTRAWIDE_ID.to_owned());
        let saved = ui.doc.source();

        ui.displays.truncate(1);
        let built_in = ui.displays[0].id.clone();
        ui.set_display_width(&built_in, Some(0));

        assert!(picked(&ui).is_none());
        assert!(
            ui.doc.source().starts_with(saved.as_str()),
            "{}",
            ui.doc.source()
        );
    }

    #[test]
    fn duplicate_and_missing_names_get_distinct_labels() {
        let mut displays = workspaces::fixture_three();
        assert_eq!(label(&displays, 1), "Studio Display 1");
        assert_eq!(label(&displays, 2), "Studio Display 2");
        displays[0].name = String::new();
        assert_eq!(label(&displays, 0), "Display 1");
    }
}
