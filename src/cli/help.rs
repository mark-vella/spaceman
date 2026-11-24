pub(crate) fn version() -> String {
    format!("spaceman {}", spaceman::build_version())
}

pub(crate) fn is_help_flag(arg: &str) -> bool {
    matches!(arg, "-h" | "--help")
}

pub(crate) fn root_usage(executable: &str) -> String {
    let mut lines = vec![
        format!("usage: {executable} <command> [args...]"),
        format!("       {executable} --help | --version"),
        String::new(),
        "Configuration commands:".into(),
        format!("  {executable} config <path|show|get|set|check> ..."),
        format!("  {executable} rules <list|add|remove> ..."),
        format!("  {executable} workspaces <list|assign|clear> ..."),
        format!("  {executable} config-check <config.toml>"),
    ];
    #[cfg(target_os = "macos")]
    {
        lines.extend([
            String::new(),
            "Runtime control & diagnostics:".into(),
            format!("  {executable} ctl <command> [args...]"),
            format!("  {executable} open <url-or-target>"),
            format!("  {executable} displays"),
            format!("  {executable} snapshot"),
            format!("  {executable} hud"),
            format!("  {executable} retile [--apply]"),
            format!("  {executable} watch <cycles> [--apply]"),
            format!("  {executable} watch-window <window-id> <cycles> [--apply]"),
            format!("  {executable} run [--apply]"),
        ]);
    }
    lines.extend([
        String::new(),
        "Inspection & fixtures:".into(),
        format!("  {executable} doctor [hours]"),
        format!("  {executable} replay <fixture.json>"),
    ]);
    lines.join("\n")
}

pub(crate) fn root(executable: &str) -> String {
    let mut sections = vec![
        format!(
            "Spaceman {} — macOS tiling window manager",
            spaceman::build_version()
        ),
        String::new(),
        format!("Usage: {executable} <command> [args...]"),
        String::new(),
        "Options:".into(),
        "  -h, --help                 Show help for spaceman or a subcommand".into(),
        "  -V, --version              Print the spaceman version".into(),
        String::new(),
        "Configuration:".into(),
        "  config <subcommand>        Inspect, validate, and edit config.toml atomically".into(),
        "  rules <subcommand>         List, add, and remove ordered window rules".into(),
        "  workspaces <subcommand>    List, assign, and clear workspace monitor routes".into(),
        "  config-check <path>        Validate a config.toml file".into(),
    ];
    #[cfg(target_os = "macos")]
    {
        sections.extend([
            String::new(),
            "Live Agent Control:".into(),
            "  ctl <command> [args...]    Send a command to the running Spaceman agent".into(),
            "  open <url-or-target>       Open Settings to a spaceman:// deep link or pane".into(),
            "  displays                   List connected displays as JSON".into(),
            "  snapshot                   Capture a dry-run window observation as JSON".into(),
            "  hud                        Stream live workspace state in the terminal".into(),
            "  retile [--apply]           Run one reconciliation cycle (dry-run by default)".into(),
            "  watch <cycles> [--apply]   Run N reconciliation cycles".into(),
            "  watch-window <id> <cycles> [--apply]".into(),
            "                             Watch a single window ID for N cycles".into(),
            "  run [--apply]              Run the foreground reconciliation loop".into(),
        ]);
    }
    sections.extend([
        String::new(),
        "Diagnostics & Testing:".into(),
        "  doctor [hours]             Summarize the arrangement journal".into(),
        "  replay <fixture.json>      Replay a recorded observation fixture".into(),
        "  help [command]             Show help for spaceman or a command".into(),
        String::new(),
        "Environment:".into(),
        "  SPACEMAN_CONFIG            Override the path to config.toml".into(),
        "  SPACEMAN_APPLICATION_SUPPORT".into(),
        "                             Override the Application Support directory".into(),
        String::new(),
        format!("Run '{executable} <command> --help' for command details."),
    ]);
    sections.join("\n")
}

pub(crate) fn subcommand(executable: &str, name: &str) -> Option<String> {
    let text = match name {
        "config" => config_help(executable),
        "rules" => rules_help(executable),
        "workspaces" => workspaces_help(executable),
        #[cfg(target_os = "macos")]
        "ctl" => ctl_help(executable),
        #[cfg(target_os = "macos")]
        "open" => format!(
            "Open Spaceman Settings on a specific pane, feature, or Lab via control.sock.\n\nUsage:\n  {executable} open <url-or-target>\n\nExamples:\n  {executable} open spaceman://settings/workspaces\n  {executable} open spaceman://labs/scrolling-strip\n  {executable} open spaceman://features/gaps\n  {executable} open rules\n\nNotes:\n  - Deep links only open Settings and highlight a target; they never mutate config.toml."
        ),
        "config-check" => format!(
            "Validate a Spaceman configuration file.\n\nUsage:\n  {executable} config-check <config.toml>"
        ),
        "doctor" => format!(
            "Summarize the arrangement journal written by the running agent.\n\nUsage:\n  {executable} doctor [hours]\n\nArguments:\n  [hours]    Optional lookback window in hours (positive integer)"
        ),
        "replay" => format!(
            "Replay a recorded observation fixture through the reconciler.\n\nUsage:\n  {executable} replay <fixture.json>"
        ),
        #[cfg(target_os = "macos")]
        "snapshot" => format!(
            "Capture a dry-run window observation and reconciliation as JSON.\n\nUsage:\n  {executable} snapshot"
        ),
        #[cfg(target_os = "macos")]
        "displays" => format!(
            "List connected displays and their IDs, names, and frames as JSON.\n\nUsage:\n  {executable} displays"
        ),
        #[cfg(target_os = "macos")]
        "hud" => format!(
            "Render a live terminal HUD of displays, workspaces, and tiles.\n\nUsage:\n  {executable} hud"
        ),
        #[cfg(target_os = "macos")]
        "retile" => format!(
            "Run one observation and reconciliation cycle.\n\nUsage:\n  {executable} retile [--apply]\n\nOptions:\n  --apply    Apply computed frames to live windows (dry-run by default)"
        ),
        #[cfg(target_os = "macos")]
        "watch" => format!(
            "Run N observation and reconciliation cycles.\n\nUsage:\n  {executable} watch <cycles> [--apply]"
        ),
        #[cfg(target_os = "macos")]
        "watch-window" => format!(
            "Run N cycles scoped to a single window ID.\n\nUsage:\n  {executable} watch-window <window-id> <cycles> [--apply]"
        ),
        #[cfg(target_os = "macos")]
        "run" => format!(
            "Run the persistent reconciliation loop in the foreground.\n\nUsage:\n  {executable} run [--apply]"
        ),
        _ => return None,
    };
    Some(text)
}

pub(crate) fn config_help(executable: &str) -> String {
    [
        "Inspect, validate, and edit Spaceman's config.toml atomically.",
        "",
        "Usage:",
        &format!("  {executable} config path [--json]"),
        &format!("  {executable} config show [--json]"),
        &format!("  {executable} config get <key> [--json]"),
        &format!("  {executable} config set <key> <value> [--json]"),
        &format!("  {executable} config check [path] [--json]"),
        "",
        "Settings keys:",
        "  hud             true | false      Show the Workspace HUD overlay (default: true)",
        "  hud_style       icons | layouts   Workspace HUD drawing style (default: icons)",
        "  update_channel  stable | dev      In-app updater release channel (default: stable)",
        "  mouse_follows_focus  off | display | window  Move pointer on focus changes (default: off)",
        "  gaps.inner      0-48              Points between tiled windows (default: 0)",
        "  gaps.top        0-128             Outer padding at the screen's top edge (default: 0;",
        "  gaps.bottom     0-128               likewise gaps.bottom, gaps.left, gaps.right)",
        "  gaps.left       0-128",
        "  gaps.right      0-128",
        "  single_window_max_width  0 | 320-16384  Center a lone window at this width (default: 0, off)",
        "",
        "Per-display overrides:",
        "  [[displays]] tables in config.toml override single_window_max_width on the displays",
        "  their monitor selects (main, secondary, a display UUID, or an exact name). 'config show'",
        "  lists them and 'config check' validates them; edit them in the file or Settings › Layout.",
        "",
        "Notes:",
        "  - Respects SPACEMAN_CONFIG and SPACEMAN_APPLICATION_SUPPORT.",
        "  - Every write is validated against version-1 schema and replaced atomically.",
    ]
    .join("\n")
}

pub(crate) fn rules_help(executable: &str) -> String {
    [
        "Manage ordered, first-match window rules in config.toml.",
        "",
        "Usage:",
        &format!("  {executable} rules list [--json]"),
        &format!(
            "  {executable} rules add --app <bundle-id> (--workspace <ws> | --float | --ignore)"
        ),
        "                         [--title <title>] [--position <index>] [--json]",
        &format!("  {executable} rules remove <index-or-bundle-id> [--json]"),
        &format!("  {executable} rules remove --app <bundle-id> [--json]"),
        "",
        "Options for 'add':",
        "  --app <bundle-id>          Match windows by macOS bundle identifier (alias: --bundle-id)",
        "  --title <title>            Require an exact window title match",
        "  --workspace <ws>           Route matching windows to workspace 1-10 or A-Z",
        "  --float                    Float matching windows without tiling",
        "  --ignore                   Ignore matching windows completely",
        "  --position <index>         Insert at 1-based rule index (default: append)",
        "  --json                     Emit machine-readable JSON output",
    ]
    .join("\n")
}

pub(crate) fn workspaces_help(executable: &str) -> String {
    [
        "Manage workspace-to-display monitor assignments in config.toml.",
        "",
        "Usage:",
        &format!("  {executable} workspaces list [--json]"),
        &format!(
            "  {executable} workspaces assign <ws> <main|secondary|display-name-or-uuid> [--json]"
        ),
        &format!("  {executable} workspaces clear <ws> [--json]"),
        "",
        "Notes:",
        "  - Workspaces 1-5 default to 'main'; workspaces 6-10 default to 'secondary'.",
        "  - Named workspaces A-Z are opt-in and default to 'main'.",
        "  - Use 'spaceman displays' to inspect display names and UUIDs.",
    ]
    .join("\n")
}

#[cfg(target_os = "macos")]
pub(crate) fn ctl_help(executable: &str) -> String {
    [
        "Send a live control command to the running Spaceman agent over control.sock.",
        "",
        "Usage:",
        &format!("  {executable} ctl <command> [args...]"),
        "",
        "Inspect:",
        "  status                     Print agent status JSON",
        "  debug                      Print live displays, tiles, focus, and targets JSON",
        "  settings                   Print agent-only settings state JSON",
        "",
        "Workspaces:",
        "  switch <workspace>         Switch to workspace 1-10 or A-Z",
        "  back-and-forth             Toggle between the current and previous workspace",
        "  move-workspace [next|prev|<display>]",
        "                             Move the focused workspace to another display (default: next)",
        "  move <workspace>           Move the focused window to a workspace",
        "  move-app <bundle-id> <ws>  Move an application's windows to a workspace",
        "",
        "Windows & layout:",
        "  focus <left|down|up|right|next>",
        "                             Move focus by direction or cycle to next window",
        "  move-direction <left|down|up|right>",
        "                             Reorder the focused window in a direction",
        "  layout                     Toggle the workspace split axis",
        "  fullscreen                 Toggle Fullscreen Layout on the active workspace",
        "  float                      Float the focused window above the tiles, or tile it again",
        "  resize <shrink|grow>       Adjust the focused window's layout weight",
        "  balance                    Reset split weights evenly",
        "  retile                     Force a fresh observation and retile",
        "",
        "Labs (need [labs] scratchpad = true):",
        "  scratchpad toggle          Summon the next scratchpad window, or hide the one shown",
        "  scratchpad mark            Send the focused window to the scratchpad, or take it out",
        "",
        "Agent & UI:",
        "  pause | resume             Pause (and restore windows) or resume tiling",
        "  wrong                      Record an anomaly marker in the arrangement journal",
        "  open <url-or-target>       Open Settings on a spaceman:// link, pane, feature, or Lab",
        "  open-settings              Open the Spaceman Settings window",
        "  open-accessibility         Open Accessibility onboarding / System Settings",
        "  check-for-updates          Check for Spaceman updates",
        "  toggle-launch-at-login     Toggle Launch at Login registration",
        "  restart | quit             Restore parked windows and exit the agent (does not relaunch)",
    ]
    .join("\n")
}
