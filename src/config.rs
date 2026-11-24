use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    DisplaySelector, ObservedWindow, WorkspaceId, Workspaces,
    shortcuts::{RawShortcuts, Shortcuts},
    spacing::{SingleWidth, Spacing, SpacingKey},
};

#[derive(Clone, Copy, Debug)]
struct AppSettings {
    hud: bool,
    hud_style: HudStyle,
    update_channel: UpdateChannel,
    mouse_follows_focus: MouseFollowsFocus,
    spacing: Spacing,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hud: true,
            hud_style: HudStyle::default(),
            update_channel: UpdateChannel::default(),
            mouse_follows_focus: MouseFollowsFocus::default(),
            spacing: Spacing::default(),
        }
    }
}

/// Which release channel the in-app updater follows: `settings.update_channel`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Public releases.
    #[default]
    Stable,
    /// A build of every merge to `dev`.
    Dev,
}

impl UpdateChannel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Dev => "dev",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "stable" => Some(Self::Stable),
            "dev" => Some(Self::Dev),
            _ => None,
        }
    }
}

/// Whether Spaceman moves the pointer when an explicit focus action lands elsewhere:
/// `settings.mouse_follows_focus`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseFollowsFocus {
    /// Leave the pointer where it is.
    #[default]
    Off,
    /// Move the pointer to the focused window (or empty display) when it is on another display.
    Display,
    /// Move the pointer to the focused window (or empty display) whenever it is outside it.
    Window,
}

impl MouseFollowsFocus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Display => "display",
            Self::Window => "window",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "display" => Some(Self::Display),
            "window" => Some(Self::Window),
            _ => None,
        }
    }
}

/// How the Workspace HUD draws each workspace in its row: `settings.hud_style`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HudStyle {
    /// Each workspace as its layout, with an app icon in every tile; the current one larger.
    Layouts,
    /// Each workspace as its number and the icons of its apps.
    #[default]
    Icons,
}

impl HudStyle {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Layouts => "layouts",
            Self::Icons => "icons",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "layouts" => Some(Self::Layouts),
            "icons" => Some(Self::Icons),
            _ => None,
        }
    }
}

/// The `[labs]` table: experimental features, each behind its own switch and off by default.
/// With every switch off Spaceman behaves exactly as if Labs did not exist.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabsConfig {
    /// The i3/sway-style Scratchpad ([`crate::scratchpad`]) and its `⌥\` / `⌥⇧\` shortcuts.
    #[serde(default)]
    pub scratchpad: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Config {
    settings: AppSettings,
    labs: LabsConfig,
    rules: Vec<WindowRule>,
    workspace_monitors: Vec<(WorkspaceId, DisplaySelector)>,
    shortcuts: Shortcuts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowDisposition {
    Managed,
    Floating,
    Route(WorkspaceId),
    /// Never tiled, owned, moved, or parked. For windows whose applications
    /// insist on managing them themselves.
    Ignored,
}

#[derive(Clone, Debug)]
struct WindowRule {
    matcher: WindowMatcher,
    action: WindowDisposition,
}

#[derive(Clone, Debug)]
struct WindowMatcher {
    bundle_id: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    version: u32,
    #[serde(default)]
    settings: RawSettings,
    #[serde(default)]
    labs: LabsConfig,
    #[serde(default)]
    rules: Vec<RawWindowRule>,
    #[serde(default)]
    workspaces: Vec<RawWorkspace>,
    #[serde(default)]
    shortcuts: RawShortcuts,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    #[serde(default = "default_hud")]
    hud: bool,
    #[serde(default)]
    hud_style: HudStyle,
    #[serde(default)]
    update_channel: UpdateChannel,
    #[serde(default)]
    mouse_follows_focus: MouseFollowsFocus,
    #[serde(default)]
    single_window_max_width: Option<toml::Value>,
    #[serde(default)]
    gaps: RawGaps,
}

impl Default for RawSettings {
    fn default() -> Self {
        Self {
            hud: true,
            hud_style: HudStyle::default(),
            update_channel: UpdateChannel::default(),
            mouse_follows_focus: MouseFollowsFocus::default(),
            single_window_max_width: None,
            gaps: RawGaps::default(),
        }
    }
}

/// `[settings.gaps]`, read wide so out-of-range values get a clear message.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGaps {
    #[serde(default)]
    inner: i64,
    #[serde(default)]
    top: i64,
    #[serde(default)]
    bottom: i64,
    #[serde(default)]
    left: i64,
    #[serde(default)]
    right: i64,
}

impl RawSettings {
    fn spacing(&self) -> Result<Spacing, String> {
        let gaps = &self.gaps;
        let mut spacing = Spacing::default();
        for (key, value) in [
            (SpacingKey::Inner, gaps.inner),
            (SpacingKey::Top, gaps.top),
            (SpacingKey::Bottom, gaps.bottom),
            (SpacingKey::Left, gaps.left),
            (SpacingKey::Right, gaps.right),
        ] {
            key.set(&mut spacing, key.check(value)?);
        }
        if let Some(width) = &self.single_window_max_width {
            spacing.single_window_max_width = SingleWidth::from_toml(width)?;
        }
        Ok(spacing)
    }
}

const fn default_hud() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindowRule {
    bundle_id: Option<String>,
    title: Option<String>,
    action: String,
    workspace: Option<WorkspaceId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkspace {
    workspace: WorkspaceId,
    monitor: String,
}

impl Config {
    #[must_use]
    #[cfg(test)]
    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }

    pub fn parse(source: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(source).map_err(|error| error.to_string())?;
        if raw.version != 1 {
            return Err(format!(
                "unsupported config version {}; expected 1",
                raw.version
            ));
        }
        let mut rules = Vec::with_capacity(raw.rules.len());
        for rule in raw.rules {
            if rule.bundle_id.is_none() && rule.title.is_none() {
                return Err("window rule must contain at least one matcher".into());
            }
            let action = match (rule.action.as_str(), rule.workspace) {
                ("float", None) => WindowDisposition::Floating,
                ("ignore", None) => WindowDisposition::Ignored,
                ("route", Some(workspace)) => WindowDisposition::Route(workspace),
                ("float", Some(_)) => return Err("float rule cannot specify workspace".into()),
                ("ignore", Some(_)) => return Err("ignore rule cannot specify workspace".into()),
                ("route", None) => return Err("route rule requires workspace".into()),
                (action, _) => return Err(format!("unknown window rule action {action:?}")),
            };
            rules.push(WindowRule {
                matcher: WindowMatcher {
                    bundle_id: rule.bundle_id,
                    title: rule.title,
                },
                action,
            });
        }
        let mut workspace_monitors = Vec::with_capacity(raw.workspaces.len());
        for workspace in raw.workspaces {
            let id = workspace.workspace;
            if workspace.monitor.trim().is_empty() {
                return Err(format!("workspace {id} monitor cannot be empty"));
            }
            if workspace_monitors
                .iter()
                .any(|(existing, _)| *existing == id)
            {
                return Err(format!("workspace {} is configured more than once", id));
            }
            workspace_monitors.push((id, DisplaySelector::parse(&workspace.monitor)));
        }
        let mut shortcuts = raw.shortcuts.parse()?;
        shortcuts.scratchpad = raw.labs.scratchpad;
        Ok(Self {
            settings: AppSettings {
                hud: raw.settings.hud,
                hud_style: raw.settings.hud_style,
                update_channel: raw.settings.update_channel,
                mouse_follows_focus: raw.settings.mouse_follows_focus,
                spacing: raw.settings.spacing()?,
            },
            labs: raw.labs,
            rules,
            workspace_monitors,
            shortcuts,
        })
    }

    /// The `[shortcuts]` table: user bindings and whether built-in shortcuts stay.
    #[must_use]
    pub const fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    /// The `[labs]` switches: experimental features, all off by default.
    #[must_use]
    pub const fn labs(&self) -> LabsConfig {
        self.labs
    }

    #[must_use]
    pub const fn hud(&self) -> bool {
        self.settings.hud
    }

    #[must_use]
    pub const fn hud_style(&self) -> HudStyle {
        self.settings.hud_style
    }

    #[must_use]
    pub const fn update_channel(&self) -> UpdateChannel {
        self.settings.update_channel
    }

    #[must_use]
    pub const fn mouse_follows_focus(&self) -> MouseFollowsFocus {
        self.settings.mouse_follows_focus
    }

    /// Gaps between tiles, outer screen padding and the lone-window width cap.
    #[must_use]
    pub const fn spacing(&self) -> Spacing {
        self.settings.spacing
    }

    pub fn load() -> Result<Self, String> {
        let path = config_path().ok_or("HOME is unavailable")?;
        match std::fs::read_to_string(&path) {
            Ok(source) => parse_path(&path, &source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("failed to read {}: {error}", path.display())),
        }
    }

    fn matching_rule(&self, bundle_id: &str, window: &ObservedWindow) -> Option<&WindowRule> {
        self.rules
            .iter()
            .find(|rule| rule.matcher.matches(bundle_id, window))
    }

    #[must_use]
    pub fn disposition(&self, bundle_id: &str, window: &ObservedWindow) -> WindowDisposition {
        self.matching_rule(bundle_id, window)
            .map_or(WindowDisposition::Managed, |rule| rule.action)
    }

    #[must_use]
    pub fn route_matches_title(&self, bundle_id: &str, window: &ObservedWindow) -> bool {
        self.matching_rule(bundle_id, window).is_some_and(|rule| {
            matches!(rule.action, WindowDisposition::Route(_)) && rule.matcher.title.is_some()
        })
    }

    #[must_use]
    pub fn has_bundle_route(&self, bundle_id: &str) -> bool {
        self.rules
            .iter()
            .find(|rule| {
                rule.matcher.title.is_none()
                    && rule
                        .matcher
                        .bundle_id
                        .as_deref()
                        .is_some_and(|value| value == bundle_id)
            })
            .is_some_and(|rule| matches!(rule.action, WindowDisposition::Route(_)))
    }

    pub fn configure_workspaces(&self, workspaces: &mut Workspaces) {
        for workspace in self.named_workspaces() {
            workspaces.enable(workspace);
        }
        for workspace in WorkspaceId::all() {
            if !workspaces.is_enabled(workspace) {
                continue;
            }
            let target = self
                .workspace_monitors
                .iter()
                .find(|(id, _)| *id == workspace)
                .map_or_else(
                    || Workspaces::default_assignment(workspace),
                    |(_, selector)| selector.clone(),
                );
            workspaces.set_assignment(workspace, target);
        }
    }

    pub fn set_workspace_monitor(&mut self, workspace: WorkspaceId, selector: DisplaySelector) {
        let should_clear = selector == Workspaces::default_assignment(workspace)
            && (workspace.number().is_some() || self.has_workspace_route(workspace));
        if should_clear {
            self.workspace_monitors
                .retain(|(existing, _)| *existing != workspace);
            return;
        }
        if let Some((_, existing)) = self
            .workspace_monitors
            .iter_mut()
            .find(|(existing, _)| *existing == workspace)
        {
            *existing = selector;
        } else {
            self.workspace_monitors.push((workspace, selector));
        }
    }

    fn has_workspace_route(&self, workspace: WorkspaceId) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.action == WindowDisposition::Route(workspace))
    }

    pub fn named_workspaces(&self) -> impl Iterator<Item = WorkspaceId> + '_ {
        self.workspace_monitors
            .iter()
            .map(|(workspace, _)| *workspace)
            .chain(self.rules.iter().filter_map(|rule| match rule.action {
                WindowDisposition::Route(workspace) => Some(workspace),
                _ => None,
            }))
            .filter(|workspace| workspace.letter().is_some())
    }
}

impl WindowMatcher {
    fn matches(&self, bundle_id: &str, window: &ObservedWindow) -> bool {
        self.bundle_id
            .as_deref()
            .is_none_or(|value| value == bundle_id)
            && self
                .title
                .as_deref()
                .is_none_or(|value| window.title.as_deref() == Some(value))
    }
}

/// Wall-clock duration since the Unix epoch.
#[must_use]
#[expect(
    clippy::disallowed_methods,
    reason = "config::now is the central wall-clock adapter"
)]
pub fn now() -> std::time::Duration {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
}

/// Optional `SPACEMAN_MANAGED_DISPLAY` override read at startup.
#[expect(
    clippy::disallowed_methods,
    reason = "startup environment variables are read in config.rs"
)]
pub fn managed_display_env() -> Result<Option<String>, String> {
    match std::env::var("SPACEMAN_MANAGED_DISPLAY") {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("SPACEMAN_MANAGED_DISPLAY must be valid UTF-8".into())
        }
    }
}

/// When the config file last changed, or `None` while it doesn't exist.
#[must_use]
pub fn modified() -> Option<std::time::SystemTime> {
    std::fs::metadata(config_path()?).ok()?.modified().ok()
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("SPACEMAN_CONFIG")
        .map(PathBuf::from)
        .or_else(|| application_support_path("config.toml"))
}

pub fn application_support_path(file: &str) -> Option<PathBuf> {
    std::env::var_os("SPACEMAN_APPLICATION_SUPPORT")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join("Library/Application Support/Spaceman"))
        })
        .map(|directory| directory.join(file))
}

pub fn load_path(path: &Path) -> Result<Config, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_path(path, &source)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EditableConfig {
    version: u32,
    #[serde(default, skip_serializing_if = "EditableSettings::is_empty")]
    settings: EditableSettings,
    /// Carried through untouched; editing a workspace never changes a Lab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    labs: Option<toml::Table>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<EditableRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    workspaces: Vec<EditableWorkspace>,
    /// Carried through untouched; this editor never changes shortcuts (Settings › Shortcuts
    /// edits them in place with `toml_edit`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shortcuts: Option<toml::Table>,
}

impl Default for EditableConfig {
    fn default() -> Self {
        Self {
            version: 1,
            settings: EditableSettings::default(),
            labs: None,
            rules: Vec::new(),
            workspaces: Vec::new(),
            shortcuts: None,
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EditableSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hud: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hud_style: Option<HudStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    update_channel: Option<UpdateChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mouse_follows_focus: Option<MouseFollowsFocus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    single_window_max_width: Option<toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gaps: Option<toml::Table>,
}

impl EditableSettings {
    const fn is_empty(&self) -> bool {
        self.hud.is_none()
            && self.hud_style.is_none()
            && self.update_channel.is_none()
            && self.mouse_follows_focus.is_none()
            && self.single_window_max_width.is_none()
            && self.gaps.is_none()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EditableRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace: Option<WorkspaceId>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EditableWorkspace {
    workspace: WorkspaceId,
    monitor: String,
}

/// Exclusive advisory lock guarding read-modify-write updates to a config file.
///
/// Locks the resolved target's parent directory via `flock(2)` so atomic renames
/// of the config file never swap the locked inode and no sidecar lock file is
/// left behind (for example inside a dotfiles repository).
pub struct ConfigLock {
    _dir: std::fs::File,
}

/// Resolves symbolic links at `path` so config locking and atomic saves operate
/// on the target file instead of replacing the symbolic link itself.
pub fn resolve_write_target(path: &Path) -> std::io::Result<PathBuf> {
    const MAX_SYMLINK_DEPTH: usize = 40;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_symlink() => return Ok(path.to_path_buf()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(path.to_path_buf());
        }
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    match std::fs::canonicalize(path) {
        Ok(resolved) => return Ok(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut current = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_DEPTH {
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::read_link(&current)?;
                current = if target.is_absolute() {
                    target
                } else {
                    let parent = current
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."));
                    let base =
                        std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
                    base.join(target)
                };
            }
            Ok(_) => return Ok(current),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(current),
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
            "too many levels of symbolic links while resolving {}",
            path.display()
        ),
    ))
}

/// Acquires an exclusive advisory lock for editing the config file at `path`.
pub fn lock_config(path: &Path) -> std::io::Result<ConfigLock> {
    let target = resolve_write_target(path)?;
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let dir = std::fs::File::open(parent)?;
    dir.lock()?;
    Ok(ConfigLock { _dir: dir })
}

/// Atomically replaces the config file at `path`, following symbolic links so a
/// symlinked `config.toml` stays linked to its updated target.
pub fn write_config_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let target = resolve_write_target(path)?;
    crate::state_file::write_atomic(&target, bytes)
}

/// Atomically updates `workspace`'s display assignment in `path`, clearing the
/// `[[workspaces]]` override when it returns to the built-in default (while
/// keeping named workspaces that have no route rule declared).
pub fn save_workspace_monitor_at(
    path: &Path,
    workspace: WorkspaceId,
    selector: &DisplaySelector,
) -> Result<Config, String> {
    let _lock =
        lock_config(path).map_err(|error| format!("failed to lock {}: {error}", path.display()))?;
    let mut doc = match std::fs::read_to_string(path) {
        Ok(source) => {
            parse_path(path, &source)?;
            toml::from_str::<EditableConfig>(&source)
                .map_err(|error| format!("{}: {error}", path.display()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => EditableConfig::default(),
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    let referenced_by_rule = doc
        .rules
        .iter()
        .any(|rule| rule.workspace == Some(workspace));
    let clear = *selector == Workspaces::default_assignment(workspace)
        && (workspace.number().is_some() || referenced_by_rule);
    if clear {
        doc.workspaces.retain(|entry| entry.workspace != workspace);
    } else {
        let monitor = selector.as_str().to_owned();
        if let Some(existing) = doc
            .workspaces
            .iter_mut()
            .find(|entry| entry.workspace == workspace)
        {
            existing.monitor = monitor;
        } else {
            doc.workspaces
                .push(EditableWorkspace { workspace, monitor });
        }
    }
    let rendered = toml::to_string_pretty(&doc)
        .map_err(|error| format!("failed to format config.toml: {error}"))?;
    let parsed = Config::parse(&rendered)
        .map_err(|error| format!("refusing to write invalid config.toml: {error}"))?;
    write_config_atomic(path, rendered.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(parsed)
}

fn parse_path(path: &Path, source: &str) -> Result<Config, String> {
    Config::parse(source).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use crate::{AppInstanceId, PhysicalWindowId, Rect};

    use super::*;

    fn window(title: Option<&str>) -> ObservedWindow {
        ObservedWindow {
            id: PhysicalWindowId {
                app: AppInstanceId {
                    pid: 1,
                    launched_at: 1,
                },
                window_id: 1,
            },
            frame: Rect::default(),
            resizable: true,
            title: title.map(std::sync::Arc::from),
        }
    }

    #[test]
    fn ordered_rules_match_all_supplied_fields() {
        let config = Config::parse(
            r#"
version = 1

[[rules]]
bundle_id = "com.example.App"
title = "Settings"
action = "float"

[[rules]]
bundle_id = "com.example.App"
action = "route"
workspace = 7
"#,
        )
        .unwrap();

        assert_eq!(
            config.disposition("com.example.App", &window(Some("Settings"))),
            WindowDisposition::Floating
        );
        assert_eq!(
            config.disposition("com.example.App", &window(Some("Document"))),
            WindowDisposition::Route(WorkspaceId::new(7).unwrap())
        );
    }

    #[test]
    fn rejects_invalid_rules_and_workspaces() {
        assert!(Config::parse("version = 2").is_err());
        assert!(Config::parse("version = 1\n[[rules]]\naction = 'float'").is_err());
        assert!(
            Config::parse(
                "version = 1\n[[rules]]\nbundle_id = 'x'\naction = 'route'\nworkspace = 11"
            )
            .is_err()
        );
        assert!(
            Config::parse(
                "version = 1\n[[rules]]\nbundle_id = 'x'\naction = 'route'\nworkspace = 'AA'"
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_named_workspace_routes_and_assignments() {
        let config = Config::parse(
            r#"
version = 1
[[rules]]
bundle_id = "ai.opencode.desktop"
action = "route"
workspace = "D"
[[workspaces]]
workspace = "D"
monitor = "main"
"#,
        )
        .unwrap();
        assert_eq!(
            config.disposition("ai.opencode.desktop", &window(None)),
            WindowDisposition::Route(WorkspaceId::named('D').unwrap())
        );
        assert_eq!(
            config.named_workspaces().collect::<Vec<_>>(),
            vec![
                WorkspaceId::named('D').unwrap(),
                WorkspaceId::named('D').unwrap()
            ]
        );
    }

    #[test]
    fn rejects_obsolete_keyboard_mapping_override() {
        assert!(Config::parse("version = 1\nkey_mapping = 'dvorak'").is_err());
    }

    #[test]
    fn parses_hud_setting_with_enabled_default() {
        assert!(Config::default().hud());
        assert!(Config::parse("version = 1").unwrap().hud());
        assert!(Config::parse("version = 1\n[settings]\n").unwrap().hud());
        assert!(
            !Config::parse("version = 1\n[settings]\nhud = false\n")
                .unwrap()
                .hud()
        );
        assert!(Config::parse("version = 1\n[settings]\nunknown = true\n").is_err());
    }

    #[test]
    fn parses_hud_style_with_icons_default() {
        assert_eq!(Config::default().hud_style(), HudStyle::Icons);
        assert_eq!(
            Config::parse("version = 1").unwrap().hud_style(),
            HudStyle::Icons
        );
        assert_eq!(
            Config::parse("version = 1\n[settings]\nhud = false\n")
                .unwrap()
                .hud_style(),
            HudStyle::Icons
        );
        for style in [HudStyle::Layouts, HudStyle::Icons] {
            let source = format!(
                "version = 1\n[settings]\nhud_style = {:?}\n",
                style.as_str()
            );
            assert_eq!(Config::parse(&source).unwrap().hud_style(), style);
            assert_eq!(HudStyle::parse(style.as_str()), Some(style));
        }
        assert!(Config::parse("version = 1\n[settings]\nhud_style = \"grid\"\n").is_err());
        assert!(Config::parse("version = 1\n[settings]\nhud_style = true\n").is_err());
        assert_eq!(HudStyle::parse("Icons"), None);
    }

    #[test]
    fn parses_labs_with_the_scratchpad_off_by_default() {
        for source in ["version = 1", "version = 1\n[labs]\n"] {
            let config = Config::parse(source).unwrap();
            assert!(!config.labs().scratchpad);
            assert!(!config.shortcuts().scratchpad);
        }
        assert!(!Config::default().labs().scratchpad);
        let config = Config::parse("version = 1\n[labs]\nscratchpad = true\n").unwrap();
        assert!(config.labs().scratchpad);
        assert!(
            config.shortcuts().scratchpad,
            "the Lab's shortcuts follow its switch"
        );
        assert!(Config::parse("version = 1\n[labs]\nwarp_drive = true\n").is_err());
        assert!(Config::parse("version = 1\n[labs]\nscratchpad = 1\n").is_err());
    }

    #[test]
    fn saving_a_workspace_monitor_keeps_the_labs_table() {
        let directory = std::env::temp_dir().join(format!(
            "spaceman-labs-{}-{}",
            std::process::id(),
            now().as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("config.toml");
        std::fs::write(&path, "version = 1\n[labs]\nscratchpad = true\n").unwrap();

        let saved = save_workspace_monitor_at(&path, WorkspaceId::ONE, &DisplaySelector::Secondary)
            .unwrap();

        assert!(saved.labs().scratchpad);
        assert!(load_path(&path).unwrap().labs().scratchpad);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn parses_update_channel_with_stable_default() {
        assert_eq!(Config::default().update_channel(), UpdateChannel::Stable);
        assert_eq!(
            Config::parse("version = 1").unwrap().update_channel(),
            UpdateChannel::Stable
        );
        for channel in [UpdateChannel::Stable, UpdateChannel::Dev] {
            let source = format!(
                "version = 1\n[settings]\nupdate_channel = {:?}\n",
                channel.as_str()
            );
            assert_eq!(Config::parse(&source).unwrap().update_channel(), channel);
            assert_eq!(UpdateChannel::parse(channel.as_str()), Some(channel));
        }
        assert!(Config::parse("version = 1\n[settings]\nupdate_channel = \"beta\"\n").is_err());
    }

    #[test]
    fn parses_mouse_follows_focus_with_off_default() {
        assert_eq!(
            Config::default().mouse_follows_focus(),
            MouseFollowsFocus::Off
        );
        assert_eq!(
            Config::parse("version = 1").unwrap().mouse_follows_focus(),
            MouseFollowsFocus::Off
        );
        for mode in [
            MouseFollowsFocus::Off,
            MouseFollowsFocus::Display,
            MouseFollowsFocus::Window,
        ] {
            let source = format!(
                "version = 1\n[settings]\nmouse_follows_focus = {:?}\n",
                mode.as_str()
            );
            assert_eq!(Config::parse(&source).unwrap().mouse_follows_focus(), mode);
            assert_eq!(MouseFollowsFocus::parse(mode.as_str()), Some(mode));
        }
        assert!(
            Config::parse("version = 1\n[settings]\nmouse_follows_focus = \"monitor\"\n").is_err()
        );
        assert!(Config::parse("version = 1\n[settings]\nmouse_follows_focus = true\n").is_err());
        assert_eq!(MouseFollowsFocus::parse("Display"), None);
    }

    #[test]
    fn saving_a_workspace_monitor_keeps_mouse_follows_focus() {
        let dir = std::env::temp_dir().join(format!(
            "spaceman-config-mff-{}-{}",
            std::process::id(),
            now().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "version = 1\n[settings]\nmouse_follows_focus = \"display\"\n",
        )
        .unwrap();
        let saved = save_workspace_monitor_at(&path, WorkspaceId::TWO, &DisplaySelector::Secondary)
            .unwrap();
        assert_eq!(saved.mouse_follows_focus(), MouseFollowsFocus::Display);
        drop(std::fs::remove_dir_all(dir));
    }

    #[test]
    fn save_workspace_monitor_persists_overrides_and_clears_built_in_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "spaceman-config-save-{}-{}",
            std::process::id(),
            now().as_nanos()
        ));
        let path = dir.join("config.toml");

        let saved = save_workspace_monitor_at(&path, WorkspaceId::ONE, &DisplaySelector::Secondary)
            .unwrap();
        let mut workspaces = Workspaces::default();
        saved.configure_workspaces(&mut workspaces);
        assert_eq!(
            workspaces.assignment(WorkspaceId::ONE),
            &DisplaySelector::Secondary
        );
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("monitor = \"secondary\""), "{written}");

        let reset =
            save_workspace_monitor_at(&path, WorkspaceId::ONE, &DisplaySelector::Main).unwrap();
        reset.configure_workspaces(&mut workspaces);
        assert_eq!(
            workspaces.assignment(WorkspaceId::ONE),
            &DisplaySelector::Main
        );
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("[[workspaces]]"), "{written}");

        let named = WorkspaceId::named('D').unwrap();
        let kept = save_workspace_monitor_at(&path, named, &DisplaySelector::Main).unwrap();
        assert_eq!(kept.named_workspaces().collect::<Vec<_>>(), vec![named]);
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("workspace = \"D\""), "{written}");

        drop(std::fs::remove_dir_all(dir));
    }

    #[test]
    fn save_workspace_monitor_preserves_symlink_target_without_sidecar_files() {
        let dir = std::env::temp_dir().join(format!(
            "spaceman-config-symlink-{}-{}",
            std::process::id(),
            now().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("dotfiles.toml");
        let link = dir.join("config.toml");
        std::fs::write(&target, "version = 1\n").unwrap();
        std::os::unix::fs::symlink("dotfiles.toml", &link).unwrap();

        save_workspace_monitor_at(&link, WorkspaceId::TWO, &DisplaySelector::Secondary).unwrap();

        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let written = std::fs::read_to_string(&target).unwrap();
        assert!(written.contains("monitor = \"secondary\""), "{written}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

        drop(std::fs::remove_dir_all(dir));
    }

    #[test]
    fn spacing_defaults_to_none_and_parses_every_key() {
        assert!(Config::default().spacing().is_none());
        assert!(Config::parse("version = 1").unwrap().spacing().is_none());
        assert!(
            Config::parse("version = 1\n[settings]\nhud = false\n")
                .unwrap()
                .spacing()
                .is_none()
        );
        let config = Config::parse(
            r#"
version = 1
[settings]
single_window_max_width = 1600
[settings.gaps]
inner = 8
top = 32
bottom = 4
left = 0
right = 128
"#,
        )
        .unwrap();
        assert_eq!(
            config.spacing(),
            Spacing {
                inner: 8,
                top: 32,
                bottom: 4,
                left: 0,
                right: 128,
                single_window_max_width: SingleWidth::from_points(1600).unwrap(),
            }
        );
        let partial = Config::parse("version = 1\n[settings.gaps]\ninner = 12\n").unwrap();
        assert_eq!(partial.spacing().inner, 12);
        assert_eq!(partial.spacing().top, 0);
    }

    #[test]
    fn spacing_rejects_out_of_range_and_unknown_keys_with_clear_messages() {
        for (source, message) in [
            (
                "[settings.gaps]\ninner = -1",
                "settings.gaps.inner must be between 0 and 48 px, got -1",
            ),
            (
                "[settings.gaps]\ninner = 49",
                "settings.gaps.inner must be between 0 and 48 px, got 49",
            ),
            (
                "[settings.gaps]\ntop = 129",
                "settings.gaps.top must be between 0 and 128 px, got 129",
            ),
            (
                "[settings.gaps]\nright = -4",
                "settings.gaps.right must be between 0 and 128 px, got -4",
            ),
            (
                "[settings]\nsingle_window_max_width = 100",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got 100",
            ),
            (
                "[settings]\nsingle_window_max_width = 99999",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got 99999",
            ),
            (
                "[settings]\nsingle_window_max_width = \"0%\"",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got \"0%\"",
            ),
            (
                "[settings]\nsingle_window_max_width = \"101%\"",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got \"101%\"",
            ),
            (
                "[settings]\nsingle_window_max_width = \"80.5%\"",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got \"80.5%\"",
            ),
            (
                "[settings]\nsingle_window_max_width = \"1680\"",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got \"1680\"",
            ),
            (
                "[settings]\nsingle_window_max_width = 0.8",
                "settings.single_window_max_width must be 0 (off), between 320 and 16384 px, or a percentage from \"1%\" to \"100%\", got 0.8",
            ),
        ] {
            let error = Config::parse(&format!("version = 1\n{source}\n")).unwrap_err();
            assert_eq!(error, message);
        }
        assert!(Config::parse("version = 1\n[settings.gaps]\nouter = 4\n").is_err());
        assert!(Config::parse("version = 1\n[settings.gaps]\ninner = 1.5\n").is_err());
    }

    #[test]
    fn single_window_max_width_takes_points_or_a_percentage() {
        let width = |source: &str| {
            Config::parse(&format!("version = 1\n[settings]\n{source}\n"))
                .unwrap()
                .spacing()
                .single_window_max_width
        };
        assert_eq!(width("single_window_max_width = 0"), SingleWidth::Off);
        assert_eq!(width("single_window_max_width = 1680").to_string(), "1680");
        assert_eq!(
            width("single_window_max_width = \"80%\"").to_string(),
            "80%"
        );
        assert_eq!(width("single_window_max_width = \"1%\"").to_string(), "1%");
        assert_eq!(
            width("single_window_max_width = \"100%\"").to_string(),
            "100%"
        );
        assert!(
            Config::parse("version = 1\n[settings]\nsingle_window_max_width_pct = 80\n").is_err()
        );
    }

    #[test]
    fn saving_a_workspace_monitor_keeps_spacing() {
        let dir = std::env::temp_dir().join(format!(
            "spaceman-config-spacing-{}-{}",
            std::process::id(),
            now().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "version = 1\n[settings]\nsingle_window_max_width = 1440\n[settings.gaps]\ninner = 6\nleft = 20\n",
        )
        .unwrap();
        let saved = save_workspace_monitor_at(&path, WorkspaceId::TWO, &DisplaySelector::Secondary)
            .unwrap();
        assert_eq!(saved.spacing().inner, 6);
        assert_eq!(saved.spacing().left, 20);
        assert_eq!(saved.spacing().single_window_max_width.to_string(), "1440");
        drop(std::fs::remove_dir_all(dir));
    }

    #[test]
    fn saving_a_workspace_monitor_keeps_shortcuts() {
        let dir = std::env::temp_dir().join(format!(
            "spaceman-config-shortcuts-{}-{}",
            std::process::id(),
            now().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "version = 1\n[shortcuts]\ndefaults = false\n[shortcuts.bindings]\n\"alt+left\" = \"move-direction left\"\n",
        )
        .unwrap();
        let saved = save_workspace_monitor_at(&path, WorkspaceId::TWO, &DisplaySelector::Secondary)
            .unwrap();
        assert!(!saved.shortcuts().defaults);
        assert_eq!(saved.shortcuts().bindings.len(), 1);
        drop(std::fs::remove_dir_all(dir));
    }

    #[test]
    fn shortcuts_default_to_built_ins_and_reject_bad_bindings() {
        assert_eq!(
            Config::parse("version = 1").unwrap().shortcuts(),
            &crate::shortcuts::Shortcuts::default()
        );
        for (source, message) in [
            (
                "[shortcuts.bindings]\n\"super+q\" = \"float\"",
                "unknown modifier",
            ),
            (
                "[shortcuts.bindings]\n\"alt+q\" = \"close\"",
                "unknown command",
            ),
            (
                "[shortcuts]\ndefaults = false",
                "needs at least one binding",
            ),
            ("[shortcuts]\nmode = \"replace\"", "unknown field"),
        ] {
            let error = Config::parse(&format!("version = 1\n{source}")).unwrap_err();
            assert!(error.contains(message), "{source}: {error}");
        }
    }

    /// A global shortcut fires in every app, so a chord without cmd, ctrl, or alt would eat
    /// ordinary typing. Unbinding is the only exception, and it never registers a chord.
    #[test]
    fn shortcuts_never_bind_a_plain_or_shift_only_key() {
        for chord in [
            "\"h\"",
            "\"shift+h\"",
            "\"shift+left\"",
            "\"shift+/\"",
            "\"shift+1\"",
            "\"space\"",
        ] {
            let source = format!("version = 1\n[shortcuts.bindings]\n{chord} = \"float\"");
            let error = Config::parse(&source).unwrap_err();
            assert!(
                error.contains("needs at least one of alt, cmd, or ctrl"),
                "{chord}: {error}"
            );
        }
        assert!(Config::parse("version = 1\n[shortcuts.bindings]\n\"alt+h\" = \"none\"").is_ok());
    }
}
