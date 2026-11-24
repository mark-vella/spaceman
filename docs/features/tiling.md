# Tiling

Arrange a workspace's windows as non-overlapping tiles in nested horizontal/vertical splits, and
navigate, reorder, nest, resize, and balance them from the keyboard or the pointer.

## Sub-features

```ts
Automatic tiling                                        // tile.auto
├── New managed window -> joins the last focused tile’s split, or the root without a surviving focus
│   └── Creation or activation before the first visible frame -> briefly recheck for the foreground window
├── Window closes -> its slot retires, siblings reflow
│   ├── Last app window closes but its surface lingers -> siblings reflow when the app confirms closure
│   └── App keeps its ordered-out window alive -> slot settles for two seconds
└── Ghostty native tab replaces its window -> same slot, order, weight, nesting

Focus                                                   // tile.focus
├── Option+H/J/K/L / ctl focus <dir> -> nearest tile in that direction, crossing displays
│   └── In Fullscreen Layout -> previous/next tile in stable order (see fullscreen-layout.md)
└── Option+` / ctl focus next        -> next tile in stable order, wrapping

Reorder                                                 // tile.reorder
├── Option+Shift+H/J/K/L / ctl move-direction <dir> -> swap/move within the tree
│   ├── At the workspace edge -> moves to the adjacent display's active workspace, even an empty one
│   └── Perpendicular move    -> re-orients or creates a mixed split
└── Pointer drag
    ├── Over a sibling     -> live-previewed swap, applied on mouse-up
    ├── Near a tile edge   -> nested split
    └── Onto another display -> moved to that display's active workspace

Size                                                    // tile.size
├── Option+- / Option+= / ctl resize shrink|grow -> 50 px at the immediate split
├── Pointer resize       -> split weights follow both edges of a corner drag, preview during the gesture
├── ctl balance          -> equalize the focused tile's immediate split (no default hotkey)
└── Window refuses its tile size (app minimum) -> its tile grows to fit and siblings give up
    the space; if necessary, try stacking an adjacent pair

Axis                                                    // tile.axis
└── Option+/ / ctl layout -> flip the root split axis (also exits Fullscreen Layout)

Spacing                                                 // tile.spacing
├── settings.gaps.inner              -> exactly that many points between neighboring tiles, any nesting
├── settings.gaps.top/bottom/left/right -> outer padding; gives way above a 320×240 tiled area
├── settings.single_window_max_width -> a lone tile (one tiled window, or Fullscreen Layout) centered
│   at that width when the padded area is wider
└── [[displays]] single_window_max_width -> that width on the displays its monitor selects (first
    match wins, 0 fills; an unplugged display's entry is inert); follows a workspace moved between
    displays
```

## How To Get To It (User POV)

```ts
Keyboard: Option+H/J/K/L, Option+Shift+H/J/K/L, Option+`, Option+-, Option+=, Option+/
Pointer:  drag a managed window (reorder/nest/move display), drag its edge (resize)
CLI:      spaceman ctl focus | move-direction | resize | balance | layout | retile
Settings: Settings › Layout -> Window gaps (Customize separately: Between windows, Screen edges, Top/Bottom/Left/Right), Display (All displays or one display), Single-window width,
          Follow All displays
Config:   spaceman config set gaps.inner|gaps.top|gaps.bottom|gaps.left|gaps.right|single_window_max_width <points>
          [[displays]] monitor + single_window_max_width in config.toml (listed by spaceman config show)
```

Input arriving while the worker is busy queues in order (up to 64); queued input is dropped on
pause, quit, or hotkey loss.

## Existing Checks

- Unit (FakeBackend), `src/app/tests/layout.rs`: `focus_navigation_follows_linear_tile_order`,
  `focus_navigation_crosses_to_the_adjacent_display`,
  `workspace_focus_cycle_wraps_through_tile_order_in_every_layout`,
  `keyboard_resize_applies_and_persists_the_focused_layout`,
  `balance_applies_and_persists_the_focused_parent_split`,
  `reorder_moves_to_the_adjacent_display_at_a_tile_boundary`,
  `reorder_moves_to_an_empty_workspace_on_the_adjacent_display` (issue 119),
  `perpendicular_reorder_creates_a_mixed_split_for_three_windows`,
  `layout_toggle_repartitions_and_persists_the_focused_workspace`.
- Engine and layout tree: `new_window_joins_the_focused_nested_split` and
  `corner_drag_moves_both_nested_boundaries` cover insertion and diagonal resizing.
- Unit (FakeBackend), `src/app/tests/gestures.rs`: drag swap preview, edge-nesting preview,
  cross-display drop; `startup.rs` covers gesture classification and `src/app/input.rs` covers
  queued final gestures.
- Unit (FakeBackend), `src/app/tests/reconciliation.rs`:
  `early_open_event_rechecks_until_the_new_window_can_split_its_sibling`,
  `confirmed_last_window_close_reflows_the_sibling_in_the_same_cycle`,
  `uncertain_close_requests_one_early_retry`,
  `recheck_reflows_when_the_ordered_out_surface_disappears`,
  `a_possible_tab_replacement_or_incomplete_snapshot_keeps_its_tile`,
  `closed_window_its_app_keeps_ordered_out_gives_up_its_tile` (an app closes its window by ordering it out),
  `workspace_whose_windows_all_leave_the_screen_keeps_its_tiles` (another Space is showing).
- Unit (FakeBackend), `src/app/tests/minimums.rs`: a window that refuses its tile width gets a wider
  tile (middle, last, several at once, and observation-cycle cases); a three-window row that cannot fit stacks two neighbors, and a wider display restores the requested row; Fullscreen Layout ignores minimums. `src/app/tests/stacked_resize.rs`: a resize while a pair is stacked keeps the saved row (issue 197). `src/layout_tree.rs`:
  `fitting_minimums_reaches_into_nested_splits_and_leaves_impossible_splits_alone`.
- Spacing, `src/spacing/tests.rs`: outer padding and its 320×240 floor; one tile, two columns and a
  nested three-tile split with inner gaps and asymmetric padding (no overlaps, exact point totals);
  equal weights give equal windows; Fullscreen Layout's capped frame; directional focus across gaps;
  a pointer resize, keyboard resize and balance through gaps. `src/config.rs` and
  `src/cli/tests.rs` cover parsing, range errors and `spaceman config`;
  `agent/src/rules/tests.rs` covers Settings › Layout saves that keep comments and restore the file.
- Per-display width: `src/config/displays/tests.rs` (parsing, each error, first match, save
  round trip), `src/app/tests/per_display.rs` (lone-window and Fullscreen Layout frames on two
  displays, a workspace moved between them, an entry for an unplugged display),
  `src/cli/tests/displays.rs` (`config show`, CLI saves, `config check`),
  `agent/src/rules/displays.rs` and `agent/src/panes/layout/displays.rs` (Settings edits that keep
  other entries and comments; playground scenes `Layout/layout-displays` and
  `Layout/layout-display-override`).
- Fixture replay: `cargo run -- replay fixtures/ghostty-tab-replacement.json` (reconciliation of a
  native-tab replacement, without the runtime).
- Live harness (not run): `scripts/live-ghostty-tabs.sh` for native-tab slot preservation.

## Gotchas

- Tests check requested frames; apps with minimum sizes may refuse them. Spaceman grows such a
  window's tile to the size it kept, taking the space from its siblings in proportion to what each
  can spare, and logs a `minimum size:` line in `agent.log`. Minimums are learned per window, only
  from refusals visible right after the write, and kept until the window closes or Spaceman
  restarts. Apps that clamp
  later (Electron aspect ratios) are still recorded as constrained instead of fought every cycle.
  If the requested split cannot fit, Spaceman tries grouping one adjacent pair of root children
  on the other axis. It checks both width and height and keeps window order. Automatic fits
  are temporary; the requested layout returns when it fits again. A resize or balance while a
  pair is stacked changes only the stack shown, never the saved split. If no pair solves the shortage,
  windows may overlap. A sustained overlap involving a constrained tile appears in `spaceman doctor`
  as `constrained_overlap`.
- Some apps (many SwiftUI apps) close a window by ordering it out; the window server keeps
  listing it. When a sibling stays visible and the app had one tile on this workspace, an empty
  Accessibility window list can confirm the close immediately. An inconclusive check gets one
  retry after 75 ms. A surface that stays ordered out then follows the usual two-second settling
  period. Minimizing an app's only window or hiding the app (Command-H) does the
  same; the window comes back as a new tile at the end of the layout. While nothing on the display
  is observed (another macOS Space is showing), tiles are kept.
- `balance` has no default hotkey; it is CLI-only.
- With gaps, a drop that lands in the gap between two tiles misses both and leaves the layout as it
  was. The Workspace HUD's layout previews stay schematic and ignore spacing.
- A `[[displays]]` name selector picks the first connected display with that name, as
  `[[workspaces]]` does; two identical monitors need their UUIDs to get different widths. Only
  `single_window_max_width` can differ per display; gaps stay global. `spaceman config set` edits
  only the global width.
- A window that enters macOS native fullscreen (for example browser video) loses its slot after two
  seconds and returns as a new tile at the end of the layout
  ([known defect](fullscreen-layout.md#gotchas)).
- Floating windows (rules, manual Toggle Float, modal/dialog windows without an enabled fullscreen
  button, or non-resizable windows) are owned by a workspace (and can be moved between workspaces)
  but are never tiled or resized. A window that already holds a tile keeps it while it temporarily
  reports non-resizable (such as while showing a window-modal close or save sheet); only a `float`
  rule or Toggle Float releases a tile.
- New windows open on the active workspace of the display under the window's center unless matched by
  an explicit `route` rule or startup placement recall.
