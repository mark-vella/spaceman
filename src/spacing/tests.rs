use super::*;
use crate::{
    AppInstanceId, Axis, Direction, Engine, NavigationTile, PhysicalWindowId, Settings, Slot,
    SlotId, WorkspaceId, layout_tree::LayoutTree,
};

const OUTPUT: Rect = Rect {
    x: 0,
    y: 25,
    width: 1000,
    height: 800,
};

fn spacing(inner: i32, [top, bottom, left, right]: [i32; 4]) -> Spacing {
    Spacing {
        inner,
        top,
        bottom,
        left,
        right,
        single_window_max_width: SingleWidth::Off,
    }
}

fn width(raw: &str) -> SingleWidth {
    SingleWidth::parse(raw).unwrap()
}

fn frames(spacing: Spacing, layout: &LayoutTree) -> Vec<(SlotId, Rect)> {
    let usable = spacing.usable(OUTPUT);
    spacing.place(usable, layout.placements(spacing.canvas(usable)))
}

fn right(rect: Rect) -> i32 {
    rect.x + rect.width
}

fn bottom(rect: Rect) -> i32 {
    rect.y + rect.height
}

/// Every frame inside `usable`, no two overlapping, and every pair beside each other at least
/// `inner` apart (the tests check that neighbors are exactly `inner` apart).
fn assert_tiled(spacing: Spacing, tiles: &[(SlotId, Rect)]) {
    let usable = spacing.usable(OUTPUT);
    for (_, frame) in tiles {
        assert!(
            frame.x >= usable.x && right(*frame) <= right(usable),
            "{frame:?}"
        );
        assert!(
            frame.y >= usable.y && bottom(*frame) <= bottom(usable),
            "{frame:?}"
        );
    }
    for (index, (_, a)) in tiles.iter().enumerate() {
        for (_, b) in &tiles[index + 1..] {
            let width = right(*a).min(right(*b)) - a.x.max(b.x);
            let height = bottom(*a).min(bottom(*b)) - a.y.max(b.y);
            assert!(width <= 0 || height <= 0, "{a:?} overlaps {b:?}");
            if height > 0 {
                let gap = (b.x - right(*a)).max(a.x - right(*b));
                assert!(gap >= spacing.inner, "{a:?} and {b:?}");
            }
            if width > 0 {
                let gap = (b.y - bottom(*a)).max(a.y - bottom(*b));
                assert!(gap >= spacing.inner, "{a:?} and {b:?}");
            }
        }
    }
}

fn nested_three() -> LayoutTree {
    // 1 on the left; 2 over 3 on the right.
    let mut layout = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(layout.move_relative(SlotId(3), SlotId(2), Direction::Down));
    layout
}

#[test]
fn outer_padding_insets_the_output_and_gives_way_before_the_minimum() {
    let padded = spacing(0, [30, 10, 20, 40]);
    assert_eq!(
        padded.usable(OUTPUT),
        Rect {
            x: 20,
            y: 55,
            width: 940,
            height: 760,
        }
    );
    // 1000 wide leaves 680 of room above the 320 minimum; 600 + 600 shares it evenly.
    let huge = spacing(0, [0, 0, 600, 600]);
    assert_eq!(huge.usable(OUTPUT).width, MIN_USABLE.0);
    assert_eq!(huge.usable(OUTPUT).x, 340);
    // An output already under the minimum keeps its whole size.
    let tiny = Rect {
        width: 300,
        height: 200,
        ..OUTPUT
    };
    assert_eq!(spacing(0, [50, 50, 50, 50]).usable(tiny), tiny);
    assert_eq!(Spacing::NONE.usable(OUTPUT), OUTPUT);
}

#[test]
fn one_tile_fills_the_usable_rect_or_centers_at_the_max_width() {
    let layout = LayoutTree::flat(Axis::Horizontal, [SlotId(1)]);
    let padded = spacing(12, [8, 8, 8, 8]);
    assert_eq!(
        frames(padded, &layout),
        [(SlotId(1), padded.usable(OUTPUT))]
    );

    let capped = Spacing {
        single_window_max_width: width("600"),
        ..spacing(12, [0, 0, 100, 0])
    };
    let usable = capped.usable(OUTPUT);
    let [(_, frame)] = frames(capped, &layout)[..] else {
        panic!("one tile")
    };
    assert_eq!(frame.width, 600);
    assert_eq!(frame.x - usable.x, right(usable) - right(frame));
    assert_eq!((frame.y, frame.height), (usable.y, usable.height));

    // Wider than the usable rect: no cap.
    let wide = Spacing {
        single_window_max_width: width("4000"),
        ..Spacing::NONE
    };
    assert_eq!(frames(wide, &layout)[0].1, OUTPUT);
}

/// A lone tile on `output` with `edge` padding on the left and right.
fn lone(raw: &str, output_width: i32, edge: i32) -> Rect {
    let spacing = Spacing {
        single_window_max_width: width(raw),
        ..spacing(8, [0, 0, edge, edge])
    };
    let output = Rect {
        width: output_width,
        ..OUTPUT
    };
    let layout = LayoutTree::flat(Axis::Horizontal, [SlotId(1)]);
    let usable = spacing.usable(output);
    spacing.place(usable, layout.placements(spacing.canvas(usable)))[0].1
}

#[test]
fn a_percentage_centers_that_share_of_the_usable_width() {
    // 3440 less 45 on each side leaves 3350; 80% is 2680.
    assert_eq!(
        lone("80%", 3440, 45),
        Rect {
            x: 380,
            y: 25,
            width: 2680,
            height: 800,
        }
    );
    // 1440 less 9 on each side leaves 1422; 80% is 1137.6, so 1137.
    assert_eq!(
        lone("80%", 1440, 9),
        Rect {
            x: 151,
            y: 25,
            width: 1137,
            height: 800,
        }
    );
}

#[test]
fn a_percentage_keeps_the_minimum_width_and_100_fills() {
    // 40% of 600 is 240, under the 320 minimum.
    assert_eq!(
        lone("40%", 600, 0),
        Rect {
            x: 140,
            y: 25,
            width: 320,
            height: 800,
        }
    );
    // A usable rect under the minimum keeps its whole width.
    assert_eq!(lone("40%", 300, 0).width, 300);
    assert_eq!(
        lone("100%", 1440, 20),
        Rect {
            x: 20,
            y: 25,
            width: 1400,
            height: 800,
        }
    );
}

#[test]
fn single_width_parses_points_and_percentages_and_prints_them_back() {
    assert_eq!(width("0"), SingleWidth::Off);
    assert_eq!(width("1680").to_string(), "1680");
    assert_eq!(width("80%").to_string(), "80%");
    for bad in [
        "319", "16385", "-5", "0%", "101%", "80.5%", "%", " 80%", "80 %", "eighty",
    ] {
        assert!(SingleWidth::parse(bad).is_err(), "{bad}");
    }
    assert_eq!(serde_json::to_string(&width("80%")).unwrap(), "\"80%\"");
    assert_eq!(serde_json::to_string(&width("1680")).unwrap(), "1680");
    assert_eq!(serde_json::to_string(&SingleWidth::Off).unwrap(), "0");
}

#[test]
fn two_columns_split_the_usable_width_less_one_gap() {
    let layout = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2)]);
    for inner in [0, 1, 7, 10, 48] {
        let gapped = Spacing {
            single_window_max_width: width("400"),
            ..spacing(inner, [30, 10, 20, 40])
        };
        let tiles = frames(gapped, &layout);
        assert_tiled(gapped, &tiles);
        let usable = gapped.usable(OUTPUT);
        let (a, b) = (tiles[0].1, tiles[1].1);
        assert_eq!(a.x, usable.x);
        assert_eq!(right(b), right(usable));
        assert_eq!(a.width + inner + b.width, usable.width);
        assert_eq!((a.y, a.height), (usable.y, usable.height));
    }
}

#[test]
fn a_nested_split_keeps_one_gap_between_every_pair_and_none_at_the_edges() {
    let layout = nested_three();
    for inner in [0, 5, 8, 48] {
        let gapped = spacing(inner, [24, 0, 6, 13]);
        let tiles = frames(gapped, &layout);
        assert_tiled(gapped, &tiles);
        let usable = gapped.usable(OUTPUT);
        let [one, two, three] = [tiles[0].1, tiles[1].1, tiles[2].1];
        assert_eq!(
            (one.x, one.y, bottom(one)),
            (usable.x, usable.y, bottom(usable))
        );
        assert_eq!(one.width + inner + two.width, usable.width);
        assert_eq!(two.height + inner + three.height, usable.height);
        assert_eq!((right(two), right(three)), (right(usable), right(usable)));
        assert_eq!(bottom(three), bottom(usable));
    }
}

#[test]
fn untile_reverses_tile_for_every_cell() {
    let gapped = spacing(9, [3, 5, 7, 11]);
    let canvas = gapped.canvas(gapped.usable(OUTPUT));
    for (_, cell) in nested_three().placements(canvas) {
        assert_eq!(gapped.untile(gapped.tile(cell)), cell);
    }
}

#[test]
fn equal_weights_give_equal_windows_with_gaps() {
    let layout = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    for inner in [6, 7, 48] {
        let gapped = spacing(inner, [0, 0, 0, 0]);
        let tiles = frames(gapped, &layout);
        assert_tiled(gapped, &tiles);
        let widths: Vec<_> = tiles.iter().map(|(_, frame)| frame.width).collect();
        assert!(
            widths.iter().max().unwrap() - widths.iter().min().unwrap() <= 1,
            "{widths:?}"
        );
        assert_eq!(widths.iter().sum::<i32>() + 2 * inner, OUTPUT.width);
    }
}

#[test]
fn fullscreen_layout_uses_the_padded_and_capped_single_frame() {
    let settings = Settings::for_output(OUTPUT).spaced(Spacing {
        single_window_max_width: width("720"),
        ..spacing(16, [10, 10, 10, 10])
    });
    let usable = settings.usable();
    let frame = settings.single();
    assert_eq!(frame.width, 720);
    assert_eq!(frame.x, usable.x + (usable.width - 720) / 2);
    assert_eq!((frame.y, frame.height), (usable.y, usable.height));
    assert_eq!(Settings::for_output(OUTPUT).single(), OUTPUT);
}

fn window(id: u32) -> PhysicalWindowId {
    PhysicalWindowId {
        app: AppInstanceId {
            pid: 1,
            launched_at: 1,
        },
        window_id: id,
    }
}

#[test]
fn directional_focus_crosses_inner_gaps() {
    for inner in [0, 12, 48] {
        let gapped = spacing(inner, [20, 20, 20, 20]);
        let tiles: Vec<_> = frames(gapped, &nested_three())
            .into_iter()
            .map(|(slot, frame)| NavigationTile {
                workspace: WorkspaceId::ONE,
                window: window(slot.0 as u32),
                frame,
            })
            .collect();
        let target = |from, direction| {
            crate::navigation::target(&tiles, &window(from), direction)
                .map(|tile| tile.window.window_id)
        };
        assert_eq!(target(1, Direction::Right), Some(2));
        assert_eq!(target(2, Direction::Left), Some(1));
        assert_eq!(target(3, Direction::Left), Some(1));
        assert_eq!(target(2, Direction::Down), Some(3));
        assert_eq!(target(3, Direction::Up), Some(2));
        assert_eq!(target(1, Direction::Left), None);
    }
}

fn engine_with(count: u64) -> (Engine, Vec<PhysicalWindowId>) {
    let mut engine = Engine::default();
    for id in 1..=count {
        engine.insert_slot(Slot {
            id: SlotId(id),
            weight: 1,
            binding: crate::BindingState::Bound(window(id as u32)),
        });
    }
    let windows = engine.bound_windows();
    (engine, windows)
}

#[test]
fn resizing_a_gapped_window_moves_the_boundary_by_the_drag() {
    let settings = Settings::for_output(OUTPUT).spaced(spacing(20, [10, 10, 10, 10]));
    let (mut engine, windows) = engine_with(2);
    let before = engine.placements(&settings);
    // At rest, the gap is not a drag.
    assert!(!engine.resize_window(&windows[0], before[0].frame, &settings));
    let dragged = Rect {
        width: before[0].frame.width + 100,
        ..before[0].frame
    };
    assert!(engine.resize_window(&windows[0], dragged, &settings));
    let after = engine.placements(&settings);
    assert_eq!(after[0].frame, dragged);
    assert_eq!(after[1].frame.x, right(dragged) + 20);
    assert_eq!(right(after[1].frame), right(settings.usable()));
}

#[test]
fn keyboard_resize_and_balance_stay_inside_the_usable_rect() {
    let settings = Settings::for_output(OUTPUT).spaced(spacing(8, [0, 0, 50, 50]));
    let (mut engine, windows) = engine_with(3);
    assert!(engine.resize_focused_tile(&windows[0], crate::ResizeDirection::Grow, 60, &settings));
    let tiles: Vec<_> = engine
        .placements(&settings)
        .into_iter()
        .map(|placement| (placement.slot, placement.frame))
        .collect();
    assert_tiled(settings.spacing, &tiles);
    assert_eq!(tiles[1].1.x - right(tiles[0].1), 8);
    assert_eq!(tiles[2].1.x - right(tiles[1].1), 8);
    assert_eq!(right(tiles[2].1), right(settings.usable()));
    assert!(engine.balance_focused_parent(&windows[0], &settings));
    let widths: Vec<_> = engine
        .placements(&settings)
        .iter()
        .map(|placement| placement.frame.width)
        .collect();
    assert!(
        widths.iter().max().unwrap() - widths.iter().min().unwrap() <= 1,
        "{widths:?}"
    );
}

#[test]
fn keys_parse_check_and_round_trip() {
    for key in SpacingKey::ALL {
        assert_eq!(SpacingKey::parse(key.path()), Some(key));
        assert_eq!(
            SpacingKey::parse(&format!("settings.{}", key.path())),
            Some(key)
        );
        let mut value = Spacing::NONE;
        key.set(&mut value, 7);
        assert_eq!(key.get(&value), 7);
    }
    assert_eq!(SpacingKey::parse("gaps.outer"), None);
    assert_eq!(SpacingKey::Inner.check(48), Ok(48));
    assert!(SpacingKey::Inner.check(49).is_err());
    assert_eq!(SpacingKey::Top.check(128), Ok(128));
    assert_eq!(spacing(0, [4, 4, 4, 4]).uniform_edges(), Some(4));
    assert_eq!(spacing(0, [4, 4, 4, 5]).uniform_edges(), None);
    assert_eq!(Spacing::NONE.with_edges(9).uniform_edges(), Some(9));
}
