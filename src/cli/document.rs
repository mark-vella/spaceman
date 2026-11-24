use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use spaceman::{
    DisplaySelector, WorkspaceId, Workspaces,
    config::{
        Config, HudStyle, MouseFollowsFocus, UpdateChannel, config_path, lock_config,
        write_config_atomic,
    },
    spacing::{Spacing, SpacingKey},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigDoc {
    pub version: u32,
    #[serde(default, skip_serializing_if = "SettingsDoc::is_empty")]
    pub settings: SettingsDoc,
    /// `[labs]`, carried through untouched: no CLI command edits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labs: Option<LabsDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspaces: Vec<WorkspaceDoc>,
    /// `[[displays]]`, carried through in file order: no CLI command edits it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub displays: Vec<DisplayDoc>,
    /// `[shortcuts]`, carried through untouched: no CLI command edits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<ShortcutsDoc>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabsDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scratchpad: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShortcutsDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, String>,
}

impl Default for ConfigDoc {
    fn default() -> Self {
        Self {
            version: 1,
            settings: SettingsDoc::default(),
            labs: None,
            rules: Vec::new(),
            workspaces: Vec::new(),
            displays: Vec::new(),
            shortcuts: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettingsDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hud: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_hud_style"
    )]
    pub hud_style: Option<HudStyle>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_update_channel"
    )]
    pub update_channel: Option<UpdateChannel>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_mouse_follows_focus"
    )]
    pub mouse_follows_focus: Option<MouseFollowsFocus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_window_max_width: Option<i64>,
    #[serde(default, skip_serializing_if = "GapsDoc::is_empty")]
    pub gaps: GapsDoc,
}

/// `[settings.gaps]` as written: only the keys the file sets.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GapsDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<i64>,
}

impl GapsDoc {
    fn is_empty(&self) -> bool {
        self.inner.is_none()
            && self.top.is_none()
            && self.bottom.is_none()
            && self.left.is_none()
            && self.right.is_none()
    }
}

impl SettingsDoc {
    fn is_empty(&self) -> bool {
        self.hud.is_none()
            && self.hud_style.is_none()
            && self.update_channel.is_none()
            && self.mouse_follows_focus.is_none()
            && self.single_window_max_width.is_none()
            && self.gaps.is_empty()
    }

    fn spacing_slot(&mut self, key: SpacingKey) -> &mut Option<i64> {
        match key {
            SpacingKey::Inner => &mut self.gaps.inner,
            SpacingKey::Top => &mut self.gaps.top,
            SpacingKey::Bottom => &mut self.gaps.bottom,
            SpacingKey::Left => &mut self.gaps.left,
            SpacingKey::Right => &mut self.gaps.right,
            SpacingKey::SingleWindowMaxWidth => &mut self.single_window_max_width,
        }
    }

    /// The spacing the file asks for, every unset key at its default `0`.
    pub(crate) fn effective_spacing(&self) -> Spacing {
        let mut doc = *self;
        let mut spacing = Spacing::NONE;
        for key in SpacingKey::ALL {
            let value = doc.spacing_slot(key).unwrap_or(0);
            key.set(&mut spacing, i32::try_from(value).unwrap_or(0));
        }
        spacing
    }

    /// Sets one spacing key after checking its range; `0` removes it, back to the default.
    pub(crate) fn set_spacing(&mut self, key: SpacingKey, value: i64) -> Result<i32, String> {
        let checked = key.check(value)?;
        *self.spacing_slot(key) = (checked != 0).then_some(i64::from(checked));
        Ok(checked)
    }

    pub(crate) fn effective_hud(&self) -> bool {
        self.hud.unwrap_or(true)
    }

    pub(crate) fn effective_hud_style(&self) -> HudStyle {
        self.hud_style.unwrap_or_default()
    }

    pub(crate) fn effective_update_channel(&self) -> UpdateChannel {
        self.update_channel.unwrap_or_default()
    }

    pub(crate) fn effective_mouse_follows_focus(&self) -> MouseFollowsFocus {
        self.mouse_follows_focus.unwrap_or_default()
    }
}

fn serialize_optional_hud_style<S: serde::Serializer>(
    style: &Option<HudStyle>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match style {
        Some(style) => serializer.serialize_str(style.as_str()),
        None => serializer.serialize_none(),
    }
}

fn serialize_optional_update_channel<S: serde::Serializer>(
    channel: &Option<UpdateChannel>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match channel {
        Some(channel) => serializer.serialize_str(channel.as_str()),
        None => serializer.serialize_none(),
    }
}

fn serialize_optional_mouse_follows_focus<S: serde::Serializer>(
    mode: &Option<MouseFollowsFocus>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match mode {
        Some(mode) => serializer.serialize_str(mode.as_str()),
        None => serializer.serialize_none(),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuleDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceDoc {
    pub workspace: WorkspaceId,
    pub monitor: String,
}

/// One `[[displays]]` entry as written.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DisplayDoc {
    pub monitor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_window_max_width: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct EffectiveSettings {
    pub hud: bool,
    pub hud_style: &'static str,
    pub update_channel: &'static str,
    pub mouse_follows_focus: &'static str,
    pub single_window_max_width: i32,
    pub gaps: EffectiveGaps,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct EffectiveGaps {
    pub inner: i32,
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RuleView {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceId>,
}

impl RuleView {
    pub(crate) fn from_doc(index: usize, rule: &RuleDoc) -> Self {
        Self {
            index,
            bundle_id: rule.bundle_id.clone(),
            title: rule.title.clone(),
            action: rule.action.clone(),
            workspace: rule.workspace,
        }
    }
}

#[derive(Serialize)]
struct EffectiveConfigToml<'a> {
    version: u32,
    settings: EffectiveSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    labs: Option<LabsDoc>,
    #[serde(skip_serializing_if = "<[RuleDoc]>::is_empty")]
    rules: &'a [RuleDoc],
    #[serde(skip_serializing_if = "<[WorkspaceDoc]>::is_empty")]
    workspaces: &'a [WorkspaceDoc],
    #[serde(skip_serializing_if = "<[DisplayDoc]>::is_empty")]
    displays: &'a [DisplayDoc],
    #[serde(skip_serializing_if = "Option::is_none")]
    shortcuts: Option<&'a ShortcutsDoc>,
}

impl ConfigDoc {
    pub(crate) fn load_or_default(path: &Path) -> Result<(Self, bool), String> {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Self::default(), false));
            }
            Err(error) => {
                return Err(format!("failed to read {}: {error}", path.display()));
            }
        };
        Config::parse(&source).map_err(|error| format!("{}: {error}", path.display()))?;
        let doc: Self =
            toml::from_str(&source).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok((doc, true))
    }

    pub(crate) fn effective_settings(&self) -> EffectiveSettings {
        let spacing = self.settings.effective_spacing();
        EffectiveSettings {
            hud: self.settings.effective_hud(),
            hud_style: self.settings.effective_hud_style().as_str(),
            update_channel: self.settings.effective_update_channel().as_str(),
            mouse_follows_focus: self.settings.effective_mouse_follows_focus().as_str(),
            single_window_max_width: spacing.single_window_max_width,
            gaps: EffectiveGaps {
                inner: spacing.inner,
                top: spacing.top,
                bottom: spacing.bottom,
                left: spacing.left,
                right: spacing.right,
            },
        }
    }

    pub(crate) fn render_toml(&self) -> Result<String, String> {
        let rendered = toml::to_string_pretty(self)
            .map_err(|error| format!("failed to format config.toml: {error}"))?;
        Config::parse(&rendered)
            .map_err(|error| format!("refusing to write invalid config.toml: {error}"))?;
        Ok(rendered)
    }

    pub(crate) fn render_effective_toml(&self) -> Result<String, String> {
        let view = EffectiveConfigToml {
            version: self.version,
            settings: self.effective_settings(),
            labs: self.labs,
            rules: &self.rules,
            workspaces: &self.workspaces,
            displays: &self.displays,
            shortcuts: self.shortcuts.as_ref(),
        };
        let rendered = toml::to_string_pretty(&view)
            .map_err(|error| format!("failed to format config.toml: {error}"))?;
        Config::parse(&rendered)
            .map_err(|error| format!("config.toml failed validation: {error}"))?;
        Ok(rendered)
    }

    pub(crate) fn save_atomic(&self, path: &Path) -> Result<(), String> {
        let rendered = self.render_toml()?;
        write_config_atomic(path, rendered.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", path.display()))
    }

    /// Acquires an exclusive config lock, reads the latest document from `path`,
    /// runs `edit`, and writes atomically when `edit` reports a change.
    pub(crate) fn edit_locked<T>(
        path: &Path,
        edit: impl FnOnce(&mut Self) -> Result<(T, bool), String>,
    ) -> Result<T, String> {
        let _lock = lock_config(path)
            .map_err(|error| format!("failed to lock {}: {error}", path.display()))?;
        let (mut doc, _) = Self::load_or_default(path)?;
        let (result, changed) = edit(&mut doc)?;
        if changed {
            doc.save_atomic(path)?;
        }
        Ok(result)
    }

    pub(crate) fn rule_views(&self) -> Vec<RuleView> {
        self.rules
            .iter()
            .enumerate()
            .map(|(index, rule)| RuleView::from_doc(index + 1, rule))
            .collect()
    }

    pub(crate) fn workspace_monitor(&self, workspace: WorkspaceId) -> Option<&str> {
        self.workspaces
            .iter()
            .find(|entry| entry.workspace == workspace)
            .map(|entry| entry.monitor.as_str())
    }

    pub(crate) fn active_workspaces(&self) -> Vec<WorkspaceId> {
        let mut out: Vec<WorkspaceId> = WorkspaceId::numbered().collect();
        let mut named = BTreeSet::new();
        for entry in &self.workspaces {
            if entry.workspace.letter().is_some() {
                named.insert(entry.workspace);
            }
        }
        for rule in &self.rules {
            if let Some(workspace) = rule.workspace
                && workspace.letter().is_some()
            {
                named.insert(workspace);
            }
        }
        out.extend(named);
        out
    }
}

pub(crate) fn resolve_config_path(override_path: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = override_path {
        return Ok(path.to_path_buf());
    }
    config_path().ok_or_else(|| "HOME is unavailable".into())
}

pub(crate) fn default_monitor_for(workspace: WorkspaceId) -> &'static str {
    match Workspaces::default().assignment(workspace) {
        DisplaySelector::Main => "main",
        DisplaySelector::Secondary => "secondary",
        DisplaySelector::Preferred(_) => "main",
    }
}

pub(crate) fn format_rule_summary(rule: &RuleDoc) -> String {
    let mut matchers = Vec::new();
    if let Some(bundle_id) = &rule.bundle_id {
        matchers.push(format!("bundle_id={bundle_id}"));
    }
    if let Some(title) = &rule.title {
        matchers.push(format!("title={title:?}"));
    }
    let target = match (rule.action.as_str(), rule.workspace) {
        ("route", Some(workspace)) => format!("workspace {workspace}"),
        (action, _) => action.to_owned(),
    };
    format!("{} -> {target}", matchers.join(" "))
}

pub(crate) fn to_json_pretty(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to encode JSON output: {error}"))
}
