//! Layout pane: one gap for everything by default, or the space between tiled windows and each
//! screen edge on its own, and the width a lone window keeps on a wide display, for every display
//! or one display at a time, under a live miniature of the result.

mod displays;
mod preview;
mod slider;

use rune::{Color, El, Tx, col, icon, row, text};
use rune_kit::SegmentedControl;
use spaceman::spacing::{Spacing, SpacingKey};

use crate::{
    Ui,
    controls::{self, Paint},
    view::scroll_body,
};

use slider::{Channel, slider};

/// The sliders' reach; `config.toml` takes more (48 between windows, 128 at an edge).
const SLIDER_MAX: i32 = 32;
/// Single-window widths offered besides Off.
const WIDTHS: [i32; 3] = [1440, 1680, 1920];

/// Every gap at once: between windows and all four screen edges. It reads the gap between
/// windows, so while the gaps differ a drag evens them out.
const ALL: Channel = Channel {
    get: |spacing| spacing.inner,
    set: |spacing, gap| Spacing {
        inner: gap,
        ..spacing.with_edges(gap)
    },
    max: SLIDER_MAX,
};

const INNER: Channel = Channel {
    get: |spacing| spacing.inner,
    set: |spacing, inner| Spacing { inner, ..spacing },
    max: SLIDER_MAX,
};

/// All four edges at once; while they differ, it shows the top edge and sets them all.
const EDGES: Channel = Channel {
    get: |spacing| spacing.uniform_edges().unwrap_or(spacing.top),
    set: Spacing::with_edges,
    max: SLIDER_MAX,
};

const EDGE_CHANNELS: [(SpacingKey, &str, Channel); 4] = [
    (SpacingKey::Top, "Top", edge(SpacingKey::Top)),
    (SpacingKey::Bottom, "Bottom", edge(SpacingKey::Bottom)),
    (SpacingKey::Left, "Left", edge(SpacingKey::Left)),
    (SpacingKey::Right, "Right", edge(SpacingKey::Right)),
];

const fn edge(key: SpacingKey) -> Channel {
    match key {
        SpacingKey::Top => Channel {
            get: |spacing| spacing.top,
            set: |spacing, top| Spacing { top, ..spacing },
            max: SLIDER_MAX,
        },
        SpacingKey::Bottom => Channel {
            get: |spacing| spacing.bottom,
            set: |spacing, bottom| Spacing { bottom, ..spacing },
            max: SLIDER_MAX,
        },
        SpacingKey::Left => Channel {
            get: |spacing| spacing.left,
            set: |spacing, left| Spacing { left, ..spacing },
            max: SLIDER_MAX,
        },
        _ => Channel {
            get: |spacing| spacing.right,
            set: |spacing, right| Spacing { right, ..spacing },
            max: SLIDER_MAX,
        },
    }
}

/// Whether the gap between windows and every screen edge match.
fn uniform(spacing: &Spacing) -> bool {
    spacing.uniform_edges() == Some(spacing.inner)
}

/// Whether the separate gap controls show: as the user left them, else while the gaps differ,
/// so a `config.toml` with its own edges opens on them.
pub(crate) fn customize_open(ui: &Ui, spacing: &Spacing) -> bool {
    ui.per_edge.unwrap_or(!uniform(spacing))
}

/// Which section or row on Layout a `spaceman://` deep link highlights, if any.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Highlight {
    Gaps,
    MaxWidth,
}

fn highlight_target(ui: &Ui) -> Option<Highlight> {
    match ui.highlight_feature.as_deref() {
        Some("gaps") => Some(Highlight::Gaps),
        Some("max-width" | "single-width") => Some(Highlight::MaxWidth),
        _ => None,
    }
}

fn highlight_style(
    p: Paint,
    target: Option<Highlight>,
    wanted: Highlight,
) -> Option<(Color, f32, Color)> {
    (target == Some(wanted)).then_some((p.k.c.layer_02, p.hair, p.k.c.border_focus))
}

fn apply_highlight(el: El, p: Paint, target: Option<Highlight>, wanted: Highlight) -> El {
    match highlight_style(p, target, wanted) {
        Some((fill, width, border)) => el.fill(fill).border(width, border),
        None => el,
    }
}

pub fn pane(ui: &Ui, p: Paint) -> El {
    let spacing = ui.spacing_draft.unwrap_or_else(|| ui.doc.spacing());
    let open = customize_open(ui, &spacing);
    let target = highlight_target(ui);
    let gaps_card = apply_highlight(
        controls::card(p, "card", gap_rows(p, &spacing, open)),
        p,
        target,
        Highlight::Gaps,
    );
    let gaps = col()
        .key("gaps")
        .gap(8.0)
        .child(controls::heading(p, "header", "Gaps", None))
        .child(gaps_card);
    let picked = displays::picked(ui);
    let (caption, current) = match &picked {
        Some(picked) => (displays::width_caption(picked), picked.width),
        None => (
            "A window alone on its display, and Fullscreen Layout, is centered at this width."
                .to_owned(),
            spacing.single_window_max_width,
        ),
    };
    let width = apply_highlight(
        controls::setting_row(
            p,
            "single-width",
            "Single-window width",
            Some((caption, p.muted())),
            width_picker(
                p,
                current,
                picked.as_ref().map(|picked| picked.display.id.clone()),
            ),
        ),
        p,
        target,
        Highlight::MaxWidth,
    );
    let wide: Vec<El> = displays::picker_row(p, ui)
        .into_iter()
        .chain([width])
        .chain(
            picked
                .as_ref()
                .and_then(|picked| displays::follow_row(p, picked)),
        )
        .collect();
    let single = picked.as_ref().map(|picked| preview::Single {
        label: picked.label.clone(),
        screen: picked.display.frame,
        width: picked.width,
    });
    scroll_body(
        "layout-body",
        col()
            .key("sections")
            .gap(24.0)
            .child(preview::card(p, spacing, single))
            .child(gaps)
            .child(controls::section(p, "wide", "Wide displays", wide)),
    )
}

/// One slider for every gap, then the disclosure; opened, the slider sets only the space between
/// windows and the screen edges follow, together and one by one.
fn gap_rows(p: Paint, spacing: &Spacing, open: bool) -> Vec<El> {
    let lead = if open {
        controls::setting_row(
            p,
            "inner",
            "Between windows",
            None,
            slider(p, "inner-slider", "Between windows", spacing.inner, INNER),
        )
    } else {
        controls::setting_row(
            p,
            "all",
            "Window gaps",
            (!uniform(spacing)).then(|| ("Dragging evens out every gap.".to_owned(), p.muted())),
            slider(p, "all-slider", "Window gaps", (ALL.get)(spacing), ALL),
        )
    };
    let mut rows = vec![lead, disclosure(p, open)];
    if !open {
        return rows;
    }
    rows.push(controls::setting_row(
        p,
        "edges",
        "Screen edges",
        spacing
            .uniform_edges()
            .is_none()
            .then(|| ("Each edge is set on its own.".to_owned(), p.muted())),
        slider(
            p,
            "edges-slider",
            "Screen edges",
            (EDGES.get)(spacing),
            EDGES,
        ),
    ));
    rows.extend(EDGE_CHANNELS.iter().map(|(key, title, channel)| {
        let name = key.gap_name().unwrap_or("edge");
        controls::setting_row(
            p,
            &format!("edge-{name}"),
            title,
            None,
            slider(
                p,
                &format!("{name}-slider"),
                &format!("{title} edge"),
                (channel.get)(spacing),
                *channel,
            ),
        )
        .pl(28.0)
    }));
    rows
}

/// The quiet row that shows or hides the separate gap controls.
fn disclosure(p: Paint, open: bool) -> El {
    let chevron = if open {
        p.k.glyph.chevron_down
    } else {
        p.k.glyph.caret_right
    };
    row()
        .key("customize")
        .role("button")
        .label("Customize separately")
        .h(32.0)
        .px(12.0)
        .gap(6.0)
        .center_y()
        .radius(p.k.radius.md)
        .press(p.k.c.pressed)
        .focusable(true)
        .focus_ring(2.0, -2.0, p.k.c.border_focus)
        .on_press(move |ui: &mut Ui| ui.per_edge = Some(!open))
        .child(icon(chevron, 12.0).key("chevron").color(p.k.c.icon_muted))
        .child(
            text("Customize separately")
                .key("label")
                .font(p.k.ty.small)
                .color(p.muted()),
        )
}

/// Off, the common widths, and a width only `config.toml` set when it holds one.
fn width_options(current: i32) -> (Vec<(String, i32)>, usize) {
    let mut options = vec![("Off".to_owned(), 0)];
    options.extend(WIDTHS.map(|width| (width.to_string(), width)));
    if !options.iter().any(|(_, width)| *width == current) {
        options.push((format!("{current} px"), current));
    }
    let selected = options
        .iter()
        .position(|(_, width)| *width == current)
        .unwrap_or(0);
    (options, selected)
}

/// The widths on offer, setting the width for all displays, or `display`'s own when given.
fn width_picker(p: Paint, current: i32, display: Option<String>) -> El {
    let (options, selected) = width_options(current);
    let labels: Vec<String> = options.iter().map(|(label, _)| label.clone()).collect();
    let widths: Vec<i32> = options.iter().map(|(_, width)| *width).collect();
    SegmentedControl::new(&labels, Some(selected))
        .fit()
        .on_pick(move |ui: &mut Ui, index, _: &mut Tx| {
            let Some(&width) = widths.get(index) else {
                return;
            };
            match &display {
                Some(display) => ui.set_display_width(display, Some(width)),
                None => {
                    let base = ui.doc.spacing();
                    ui.set_spacing(Spacing {
                        single_window_max_width: width,
                        ..base
                    });
                }
            }
        })
        .build(p.k)
        .key("width-picker")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_picker_offers_off_the_presets_and_a_custom_width() {
        let (options, selected) = width_options(0);
        assert_eq!(options.len(), 4);
        assert_eq!(selected, 0);
        let (_, selected) = width_options(1680);
        assert_eq!(selected, 2);
        let (options, selected) = width_options(1600);
        assert_eq!(options.len(), 5);
        assert_eq!(options[selected], ("1600 px".to_owned(), 1600));
    }

    #[test]
    fn edge_channels_write_only_their_edge() {
        let base = Spacing::NONE.with_edges(4);
        for (key, _, channel) in EDGE_CHANNELS {
            let moved = (channel.set)(base, 20);
            assert_eq!(key.get(&moved), 20);
            assert_eq!((channel.get)(&moved), 20);
            for other in SpacingKey::EDGES.into_iter().filter(|other| *other != key) {
                assert_eq!(other.get(&moved), 4);
            }
        }
        assert_eq!((EDGES.set)(base, 9).uniform_edges(), Some(9));
        assert_eq!((INNER.set)(base, 9).inner, 9);
    }

    #[test]
    fn the_locked_slider_moves_every_gap_together() {
        let mixed = Spacing {
            inner: 6,
            top: 30,
            ..Spacing::NONE
        };
        assert!(!uniform(&mixed));
        let locked = (ALL.set)(mixed, 12);
        assert!(uniform(&locked));
        assert_eq!((ALL.get)(&locked), 12);
        assert_eq!(locked.uniform_edges(), Some(12));
        assert!(uniform(&Spacing::NONE));
        let wide = Spacing {
            single_window_max_width: 1680,
            ..Spacing::NONE
        };
        assert_eq!((ALL.set)(wide, 8).single_window_max_width, 1680);
    }

    #[test]
    fn gaps_deep_link_highlights_gaps_while_ordinary_layout_link_does_not() {
        use spaceman::deep_link::parse_url;

        let p = Paint::new(true, 2.0);
        let mut ui = crate::playground::blank_fixture();

        ui.open_target(parse_url("spaceman://features/gaps").unwrap().resolve());
        assert_eq!(ui.pane, crate::Pane::Layout);
        let target = highlight_target(&ui);
        assert_eq!(target, Some(Highlight::Gaps));
        assert_eq!(
            highlight_style(p, target, Highlight::Gaps),
            Some((p.k.c.layer_02, p.hair, p.k.c.border_focus))
        );
        assert_eq!(highlight_style(p, target, Highlight::MaxWidth), None);
        let _ = pane(&ui, p);

        ui.open_target(parse_url("spaceman://settings/layout").unwrap().resolve());
        assert_eq!(ui.pane, crate::Pane::Layout);
        let target = highlight_target(&ui);
        assert_eq!(target, None);
        assert_eq!(highlight_style(p, target, Highlight::Gaps), None);
        assert_eq!(highlight_style(p, target, Highlight::MaxWidth), None);
        let _ = pane(&ui, p);

        ui.open_target(
            parse_url("spaceman://features/max-width")
                .unwrap()
                .resolve(),
        );
        assert_eq!(ui.pane, crate::Pane::Layout);
        let target = highlight_target(&ui);
        assert_eq!(target, Some(Highlight::MaxWidth));
        assert_eq!(highlight_style(p, target, Highlight::Gaps), None);
        assert_eq!(
            highlight_style(p, target, Highlight::MaxWidth),
            Some((p.k.c.layer_02, p.hair, p.k.c.border_focus))
        );
        let _ = pane(&ui, p);
    }
}
