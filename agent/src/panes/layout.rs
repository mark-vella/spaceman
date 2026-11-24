//! Layout pane: one gap for everything by default, or the space between tiled windows and each
//! screen edge on its own, and the width a lone window keeps on a wide display, under a live
//! miniature of the result.

mod preview;
mod slider;

use rune::{Color, El, col, icon, row, text};
use spaceman::spacing::{SingleWidth, Spacing, SpacingKey, WidthPercent};

use crate::{
    Ui,
    controls::{self, Paint},
    view::scroll_body,
};

use slider::{Channel, slider};

/// The sliders' reach; `config.toml` takes more (48 between windows, 128 at an edge).
const SLIDER_MAX: i32 = 32;
/// Every gap at once: between windows and all four screen edges. It reads the gap between
/// windows, so while the gaps differ a drag evens them out.
const ALL: Channel = Channel::points(
    |spacing| spacing.inner,
    |spacing, gap| Spacing {
        inner: gap,
        ..spacing.with_edges(gap)
    },
    SLIDER_MAX,
);

const INNER: Channel = Channel::points(
    |spacing| spacing.inner,
    |spacing, inner| Spacing { inner, ..spacing },
    SLIDER_MAX,
);

/// All four edges at once; while they differ, it shows the top edge and sets them all.
const EDGES: Channel = Channel::points(
    |spacing| spacing.uniform_edges().unwrap_or(spacing.top),
    Spacing::with_edges,
    SLIDER_MAX,
);

/// The single-window width as a share of the display, 40% to 100% in steps of 5, where 100% is
/// off. A width in points that only `config.toml` sets reads as itself until the slider moves,
/// its thumb at that width's share of the preview's ultrawide.
const WIDTH: Channel = Channel {
    get: |spacing| match spacing.single_window_max_width {
        SingleWidth::Off => 100,
        SingleWidth::Percent(percent) => percent.get(),
        SingleWidth::Points(points) => {
            let usable = spacing.usable(preview::ULTRAWIDE).width.max(1);
            (points.get() * 100 / usable + 2) / 5 * 5
        }
    },
    set: |spacing, percent| Spacing {
        single_window_max_width: match WidthPercent::new(i64::from(percent)) {
            Some(share) if percent < 100 => SingleWidth::Percent(share),
            _ => SingleWidth::Off,
        },
        ..spacing
    },
    min: 40,
    max: 100,
    step: 5,
    readout: |spacing, _| match spacing.single_window_max_width {
        SingleWidth::Off => "Off".to_owned(),
        SingleWidth::Points(points) => format!("{} px", points.get()),
        width @ SingleWidth::Percent(_) => width.to_string(),
    },
};

const EDGE_CHANNELS: [(SpacingKey, &str, Channel); 4] = [
    (SpacingKey::Top, "Top", edge(SpacingKey::Top)),
    (SpacingKey::Bottom, "Bottom", edge(SpacingKey::Bottom)),
    (SpacingKey::Left, "Left", edge(SpacingKey::Left)),
    (SpacingKey::Right, "Right", edge(SpacingKey::Right)),
];

const fn edge(key: SpacingKey) -> Channel {
    match key {
        SpacingKey::Top => Channel::points(
            |spacing| spacing.top,
            |spacing, top| Spacing { top, ..spacing },
            SLIDER_MAX,
        ),
        SpacingKey::Bottom => Channel::points(
            |spacing| spacing.bottom,
            |spacing, bottom| Spacing { bottom, ..spacing },
            SLIDER_MAX,
        ),
        SpacingKey::Left => Channel::points(
            |spacing| spacing.left,
            |spacing, left| Spacing { left, ..spacing },
            SLIDER_MAX,
        ),
        _ => Channel::points(
            |spacing| spacing.right,
            |spacing, right| Spacing { right, ..spacing },
            SLIDER_MAX,
        ),
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
    let width = apply_highlight(
        controls::setting_row(
            p,
            "single-width",
            "Single-window width",
            Some((
                "A window alone on its display, and Fullscreen Layout, is centered at this share \
                 of the display's width."
                    .to_owned(),
                p.muted(),
            )),
            slider(p, "width-slider", "Single-window width", &spacing, WIDTH),
        ),
        p,
        target,
        Highlight::MaxWidth,
    );
    scroll_body(
        "layout-body",
        col()
            .key("sections")
            .gap(24.0)
            .child(preview::card(p, spacing))
            .child(gaps)
            .child(controls::section(p, "wide", "Wide displays", vec![width])),
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
            slider(p, "inner-slider", "Between windows", spacing, INNER),
        )
    } else {
        controls::setting_row(
            p,
            "all",
            "Window gaps",
            (!uniform(spacing)).then(|| ("Dragging evens out every gap.".to_owned(), p.muted())),
            slider(p, "all-slider", "Window gaps", spacing, ALL),
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
        slider(p, "edges-slider", "Screen edges", spacing, EDGES),
    ));
    rows.extend(EDGE_CHANNELS.iter().map(|(key, title, channel)| {
        let name = key.gap_name();
        controls::setting_row(
            p,
            &format!("edge-{name}"),
            title,
            None,
            slider(
                p,
                &format!("{name}-slider"),
                &format!("{title} edge"),
                spacing,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn width(raw: &str) -> Spacing {
        Spacing {
            single_window_max_width: SingleWidth::parse(raw).unwrap(),
            ..Spacing::NONE
        }
    }

    #[test]
    fn the_width_slider_sets_a_share_and_100_turns_it_off() {
        let moved = (WIDTH.set)(width("1920"), 80);
        assert_eq!(moved.single_window_max_width.to_string(), "80%");
        assert_eq!((WIDTH.readout)(&moved, 80), "80%");
        let off = (WIDTH.set)(width("80%"), 100);
        assert_eq!(off.single_window_max_width, SingleWidth::Off);
        assert_eq!((WIDTH.readout)(&off, 100), "Off");
        assert_eq!((WIDTH.get)(&off), 100);
    }

    #[test]
    fn the_width_slider_shows_a_points_width_as_written() {
        let points = width("1920");
        assert_eq!((WIDTH.readout)(&points, (WIDTH.get)(&points)), "1920 px");
        assert_eq!((WIDTH.get)(&points), 55);
        assert_eq!((WIDTH.get)(&width("33%")), 33);
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
        let wide = width("1680");
        assert_eq!(
            (ALL.set)(wide, 8).single_window_max_width.to_string(),
            "1680"
        );
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
