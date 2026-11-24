# Configuration

Spaceman loads configuration when `spaceman-agent` starts and reloads it automatically whenever the
file changes on disk:

```text
~/Library/Application Support/Spaceman/config.toml
```

Set `SPACEMAN_CONFIG` in the agent's environment to replace that path. A missing file means default
configuration. Invalid configuration at startup is reported to stderr for terminal launches and to
`~/Library/Application Support/Spaceman/agent.log` for app-bundle launches; Spaceman falls back to
defaults and ignores workspace state so it can prioritize restoring parked windows. If an edit saved
while running fails validation, the running agent logs the error and keeps the previous valid
configuration.

Validate a file before replacing or restarting the app:

```sh
spaceman config-check "$HOME/Library/Application Support/Spaceman/config.toml"
```

The complete example is [config.example.toml](../config.example.toml).

## Current Schema

The parser rejects unknown fields. The root must contain `version = 1` and may contain an optional
`[settings]` table, an optional `[shortcuts]` table (see [Shortcuts](#shortcuts)), an optional
`[labs]` table (see [Labs](#labs)), plus repeated `[[rules]]` and `[[workspaces]]` tables.

```toml
version = 1

[settings]
hud = true                            # optional (default: true); non-activating Workspace HUD
hud_style = "icons"                   # optional (default: "icons"); or "layouts"
update_channel = "stable"             # optional (default: "stable"); or "dev", a build of every merge
mouse_follows_focus = "off"           # optional (default: "off"); or "display", "window"
single_window_max_width = 0           # optional (default: 0, off); 320 through 16384 points, or "1%" through "100%"

[settings.gaps]                       # optional; every key defaults to 0
inner = 0                             # points between tiled windows, 0 through 48
top = 0                               # outer padding at each screen edge, 0 through 128
bottom = 0
left = 0
right = 0

[[rules]]
bundle_id = "com.example.Application" # optional exact match
title = "Preferences"                 # optional exact match
action = "float"                      # required: "float", "route", or "ignore"

[[rules]]
bundle_id = "com.tinyspeck.slackmacgap"
action = "route"
workspace = 2                         # required for route; forbidden for float and ignore

[[rules]]
bundle_id = "ai.opencode.desktop"
action = "route"
workspace = "D"                       # named workspace A through Z

[[workspaces]]
workspace = 9                         # 1 through 10 or "A" through "Z"; unique
monitor = "main"                      # non-empty selector
```

A rule needs at least one of `bundle_id` or `title`. If both are present, both must match exactly.
Rules run in file order and the first matching rule wins. An unmatched window is managed normally.

- `float` excludes the window from tiling and resizing, preserves its native frame, and keeps it with
  the logical workspace where it was first observed.
- `route` assigns a newly inserted resizable logical window to workspace 1 through 10 or a configured
  named workspace A through Z. A bundle-level `route` rule (without `title`) applies to the
  application's initial window while secondary windows opened later stay on the active workspace; a
  `title`-matched `route` rule routes every matching window. Movable
  non-resizable windows remain floating on the workspace where first observed. A rule does not
  repeatedly route existing slots, including a Ghostty native-tab physical replacement.
- `ignore` leaves the window entirely alone: never tiled, never owned by a workspace, never moved or
  parked. Use it for applications that insist on positioning their own windows. A previously managed
  window is released the next time it is observed on screen.

During startup recovery, after a qualifying mid-session relaunch, or for an application marked to
return, an unmatched window may return to a remembered workspace; see
[Remembered Placements](#remembered-placements). Matching `route` and `ignore` rules always win over
memory.

Title matching occurs only in memory. Titles are not serialized into persisted state, snapshots,
`ctl debug`, or the HUD.

## Gaps, Screen Edges, and Single-Window Width

`[settings.gaps]` spaces tiles out. `inner` is the distance between neighboring tiles, in every split
and at every nesting level; `top`, `bottom`, `left`, and `right` pad the tiled area away from each
screen edge (for example, room for a custom menu bar on top). Outer padding gives way before the
tiled area would get smaller than 320 × 240 points. Equal weights still give equal windows with gaps.

`single_window_max_width` caps the width of a window that is alone on its display, either the only
tiled window of a workspace or any window in Fullscreen Layout. The window is centered at that width
inside the padded area; on a display that is narrower, it fills the area as before. It suits wide and
ultrawide displays, where one full-width window is hard to read.

The width is either a whole number of points (`1680`) or a quoted whole percentage of the padded
area's width on the window's own display (`"80%"`), so one value suits displays of different
sizes. A percentage never makes the window narrower than 320 points unless the padded area itself is
narrower, and `"100%"` fills the area like `0`. On a 3440-point display with 45 points of padding on
the left and right, `"80%"` gives a window 2680 points wide.

Out-of-range values are rejected with a message naming the key and its accepted forms, like any
other invalid configuration. Every key defaults to `0`, so a file without them tiles exactly as
before.

Settings › Layout edits the same keys over a live miniature of the result. One **Window gaps**
slider sets `inner` and all four edges together; **Customize separately** (open on its own when the
file's gaps differ) sets between windows, all screen edges, and each edge on its own. The
**Single-window width** slider sets a percentage from 40% to 100% in steps of 5, where 100% is off;
a width in points from the file shows as written (for example `1920 px`) until the slider moves,
which saves a percentage. Saving keeps the form the file holds and its comments.
`spaceman config` reads and writes them by dotted key:

```sh
spaceman config set gaps.inner 8
spaceman config set gaps.top 32
spaceman config set single_window_max_width 1680
spaceman config set single_window_max_width 80%
spaceman config get gaps.inner
```

`config get single_window_max_width` prints `80%` for a percentage. With `--json`, `config get` and
`config show` give a percentage as the string `"80%"` and points as a number.

## Shortcuts

`[shortcuts]` rebinds Spaceman's global shortcuts. Each entry in `[shortcuts.bindings]` maps a chord
to a command in `spaceman ctl` syntax:

```toml
[shortcuts]
defaults = true                       # optional (default: true); false registers only these bindings
modifier = "alt"                      # optional (default: "alt"); every built-in shortcut holds it

[shortcuts.bindings]
"alt+left" = "move-direction left"     # replaces macOS's Option+arrow word jump everywhere
"alt+shift+ctrl+left" = "focus left"
"alt+v" = "float"
"alt+j" = "layout"                    # replaces the built-in Option+J (focus down)
"alt+shift+b" = "balance"
"alt+h" = "none"                      # unbinds the built-in Option+H
```

`modifier` moves every built-in shortcut at once: `"ctrl+alt"` makes Switch to workspace 3
`Control+Option+3` and Move to workspace 3 `Control+Option+Shift+3`. It takes the same modifier names
as a chord, needs `alt`, `cmd`, or `ctrl`, and can't include `shift`, which the built-in Move
shortcuts add. Bindings are exact chords and don't follow it.

A chord is one or more modifiers and one key, joined by `+`, in any order and case:

- modifiers: `alt` (also `opt`, `option`), `cmd` (`command`), `ctrl` (`control`), and `shift`.
  Every chord needs `alt`, `cmd`, or `ctrl`.
- keys: a single character such as `j`, `/`, `-`, `=`, or `` ` `` (found on whatever key types it in
  the active keyboard layout, like the built-in shortcuts); `1` through `9` and `0` (the physical
  number row, so `0` is workspace 10); `left`, `right`, `up`, `down`, `return` (`enter`), `tab`,
  `space`, `escape` (`esc`), `backspace` (`delete`), `forwarddelete`, `home`, `end`, `pageup`,
  `pagedown`, and `f1` through `f12`. `alt++` is the `+` key.

Commands:

| Command | Action |
| --- | --- |
| `switch WORKSPACE` | Switch to workspace 1 through 10 or an enabled named workspace A through Z |
| `move WORKSPACE` | Move the focused window there without switching |
| `back-and-forth` | Return to the previous workspace |
| `move-workspace [next\|prev\|DISPLAY]` | Move the focused workspace to another display |
| `focus left\|down\|up\|right` | Focus the nearest tile in that direction |
| `focus next` | Focus the next tiled window in the workspace |
| `move-direction left\|down\|up\|right` | Move the focused tile in that direction |
| `layout` | Toggle the active layout's root split axis |
| `fullscreen` | Toggle Fullscreen Layout |
| `float` | Float the focused window, or tile it again |
| `resize shrink\|grow` | Shrink or grow the focused tile |
| `balance` | Reset the active workspace's split weights |
| `retile` | Retile now |
| `none` | Unbind the chord: no built-in shortcut claims it |

A global shortcut fires in every app, so a chord that types something takes that key everywhere:
`alt+left` replaces macOS's Option+arrow word jump in text fields. Prefer `ctrl` or `alt+shift` for
rebinds that must not disturb typing.

Bindings claim their chords before the built-in shortcuts, so a binding on a built-in chord replaces
it and the other built-ins stay. With `defaults = false` only the listed bindings register, and the
table must bind at least one command (tiling needs at least one shortcut to start). Two spellings of
the same chord, an unknown modifier, key, or command, or a chord without `alt`, `cmd`, or `ctrl` is a
config error. A binding whose workspace is not enabled, or that the keyboard layout cannot type, is
skipped and logged in `agent.log` while the other shortcuts register.

Shortcut changes apply live: saving `config.toml` registers the new chords within a couple of
seconds, without a restart. If the new table can't be registered, the previous shortcuts keep working
and Settings shows `Shortcuts not applied` with the reason. Workspaces enabled by a config edit still
get their shortcuts on the next start. The menu-bar workspace list keeps showing the built-in
`Option`+number/letter hints next to each workspace.

Settings › Shortcuts edits the same table. Click a shortcut's keys and press a new chord: it is saved
to `[shortcuts.bindings]` and registered at once, and the built-in chord it replaces gets `none`.
While Settings records, every global shortcut is held off so the chord reaches it (for at most a
minute). `esc` cancels, `⌫` or `Delete` on its own clears the shortcut (`none`), a chord without
`⌥`, `⌃`, or `⌘` is refused, and so is a chord another shortcut uses, naming it. Base modifier
sets `modifier`, and Reset to Defaults removes the `[shortcuts]` table. Comments and other bindings
in the file are kept.

## Labs

`[labs]` turns on experimental features one by one. Every switch is off by default, and with all of
them off Spaceman behaves exactly as if Labs did not exist: no extra shortcuts register and no
command changes behavior. Unknown keys are config errors.

```toml
[labs]
scratchpad = false                    # optional (default: false); the i3/sway-style Scratchpad
```

`scratchpad = true` enables the Scratchpad (see [Controls › Scratchpad](controls.md#scratchpad-lab)) and
registers its two shortcuts, `Option+\` and `Option+Shift+\`, live without a restart (or on the next
hotkey registration if tiling is paused); a `[shortcuts]` binding on either chord takes it instead. The
`spaceman ctl scratchpad` commands follow the switch live. Turning the switch off while windows are
hidden in the scratchpad brings them back on screen, where they tile again.

## Remembered Placements

Window identity in workspace state is tied to a running process, so it cannot survive a reboot or an
application relaunch. Spaceman therefore also remembers, per application bundle identifier, which
workspace each managed window lived on, whether it floated, and its last on-screen frame. It never
records window titles.

A new application process that owns no windows yet may claim its remembered placements in three
cases:

- **Startup recovery:** during the first 60 seconds after Spaceman starts, when an application
  bundle is seen for the first time in the session (such as after a reboot or a Spaceman start
  without valid workspace state).
- **Mid-session relaunch:** after the startup window, when another process of the same application
  quit within the last 30 seconds or still owns windows (for example, an in-app update or restart
  that launches the new process before the old one exits).
- **Returning applications:** when you move a newly opened window to another workspace by hand
  within 120 seconds of it opening on the active workspace, Spaceman marks that application to
  return so later launches recall its remembered placements as well.

Matching `route` and `ignore` rules always take precedence over memory (an application with a
bundle-level `route` rule does not open a recall, and per-window `route` and `ignore` rules skip
recall). An ordinary later launch without a qualifying relaunch or returning mark opens on the
active workspace instead of recalling historical placements.

During a 60-second recall window, eligible windows claim remembered placements: first a window whose
frame matches a remembered frame within 8 pixels, then the remaining windows in creation order. A
claimed window joins its remembered workspace (hidden if that workspace is not visible, and floating
again if it was saved floating). Windows beyond the remembered count, windows opened after the
recall window closes, and placements on workspaces that are no longer enabled or managed are adopted
on the active workspace. Multi-window applications that restore windows with identical frames are
matched by creation order, which is a best guess.

A changed arrangement replaces an application's memory only after it holds for 10 seconds while the
application keeps running, so the windows closing one by one as an application quits do not erase
the arrangement it should return to. Closing every window leaves the previous memory in place.
Memory is advisory: an unreadable or invalid file is logged and ignored. It holds up to 32 windows for
each of 128 applications, evicting the least recently changed application.

## Display Assignments

Defaults assign workspaces 1-5 to `main` and 6-10 to `secondary`. `monitor` accepts:

- `main`, the display macOS marks as main;
- `secondary`, the first attached non-main display;
- an exact display UUID from `spaceman displays`;
- an exact display name from `spaceman displays`.

```sh
spaceman displays
```

A missing `secondary`, UUID, or name falls back to the main display. Spaceman retains the preferred
assignment as its own active lane; reconnecting that display makes its assigned active workspace
visible again.

Settings › Workspaces offers every connected display: `Main`, `Secondary`, then each further
display, whose assignment it writes as that display's UUID (the same selector
`Option+Shift+Tab` persists). A saved UUID renders back as its display's name, and a saved display
that is unplugged keeps a `Disconnected displays` card so its assignment is never lost.

Named workspaces are opt-in: reference a letter in a route or `[[workspaces]]` entry to enable it.
They default to `main`. An enabled letter registers `Option+letter` to switch and
`Option+Shift+letter` to move the focused window there. The named workspace shortcut takes
precedence over a built-in letter shortcut when they overlap. A named workspace still referenced by
persisted active/previous history, tiled or floating windows, or Fullscreen Layout remains enabled
across config edits until that state no longer refers to it.

## Process Scope

`SPACEMAN_MANAGED_DISPLAY` is an internal process-scope override used by isolated tests. It accepts
`main`, `secondary`, or `both` (the default). It is not a TOML field, and the installed app does not
provide a UI for setting it. A scoped process registers only workspace hotkeys assigned to its managed
main/secondary selectors; normal users should leave it unset.

`SPACEMAN_APPLICATION_SUPPORT` is another internal test override. It replaces the Application Support
directory for the default config path, state, logs, and control files so live harnesses cannot touch
normal recovery data.

## Applying Changes

Edits saved in Spaceman Settings or in an external editor take effect automatically: rule changes,
workspace display assignments, `settings.hud`, `settings.hud_style`,
`settings.mouse_follows_focus`, `settings.gaps`, and
`settings.single_window_max_width` reload without restarting (Settings › Layout retiles at once; an
external edit applies on the next retile);
`settings.update_channel` applies at the next update check. Global shortcuts for a
newly added named workspace letter (`Option+A` ... `Option+Z`) register on the next app start:

```sh
spaceman ctl pause
spaceman ctl status
spaceman ctl quit
open "/Applications/Spaceman.app"
```

`spaceman ctl restart` safely restores parked windows and exits, but currently does not relaunch the
app by itself. The source installer uses it before replacing the bundle, then opens the new bundle.
