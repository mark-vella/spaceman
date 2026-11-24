use std::path::{Path, PathBuf};

use serde::Serialize;
use spaceman::{
    config::{HudStyle, MouseFollowsFocus, UpdateChannel, load_path},
    spacing::SpacingKey,
};

use super::{
    Outcome,
    document::{
        ConfigDoc, DisplayDoc, EffectiveSettings, LabsDoc, RuleView, ShortcutsDoc, WorkspaceDoc,
        resolve_config_path, to_json_pretty,
    },
    help,
};

#[derive(Serialize)]
struct PathOutput {
    path: String,
    exists: bool,
}

#[derive(Serialize)]
struct ShowOutput {
    path: String,
    exists: bool,
    version: u32,
    settings: EffectiveSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    labs: Option<LabsDoc>,
    rules: Vec<RuleView>,
    workspaces: Vec<WorkspaceDoc>,
    displays: Vec<DisplayDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shortcuts: Option<ShortcutsDoc>,
}

#[derive(Serialize)]
struct SettingOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    key: &'static str,
    value: serde_json::Value,
}

#[derive(Serialize)]
struct CheckOutput {
    path: String,
    exists: bool,
    valid: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingKey {
    Hud,
    HudStyle,
    UpdateChannel,
    MouseFollowsFocus,
    Spacing(SpacingKey),
}

impl SettingKey {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.strip_prefix("settings.").unwrap_or(raw) {
            "hud" => Ok(Self::Hud),
            "hud_style" => Ok(Self::HudStyle),
            "update_channel" => Ok(Self::UpdateChannel),
            "mouse_follows_focus" => Ok(Self::MouseFollowsFocus),
            other => SpacingKey::parse(other).map(Self::Spacing).ok_or_else(|| {
                format!(
                    "unknown setting {raw:?}; valid keys: hud, hud_style, update_channel, \
                     mouse_follows_focus, gaps.inner, gaps.top, gaps.bottom, gaps.left, \
                     gaps.right, single_window_max_width"
                )
            }),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Hud => "hud",
            Self::HudStyle => "hud_style",
            Self::UpdateChannel => "update_channel",
            Self::MouseFollowsFocus => "mouse_follows_focus",
            Self::Spacing(key) => key.path(),
        }
    }
}

pub(crate) fn run(executable: &str, args: &[&str], path_override: Option<&Path>) -> Outcome {
    let Some((&sub, rest)) = args.split_first() else {
        return Outcome::usage(help::config_help(executable));
    };
    if help::is_help_flag(sub) || sub == "help" || rest.iter().any(|arg| help::is_help_flag(arg)) {
        return Outcome::ok(help::config_help(executable));
    }

    let result = match sub {
        "path" => run_path(rest, path_override),
        "show" => run_show(rest, path_override),
        "get" => run_get(rest, path_override),
        "set" => run_set(rest, path_override),
        "check" => run_check(rest, path_override),
        _ => {
            return Outcome::usage(format!(
                "unknown config subcommand: {sub}\n\n{}",
                help::config_help(executable)
            ));
        }
    };

    match result {
        Ok(output) => Outcome::ok(output),
        Err(error) => Outcome::err(error),
    }
}

fn split_json_flag<'a>(args: &[&'a str]) -> Result<(bool, Vec<&'a str>), String> {
    let mut json = false;
    let mut positional = Vec::new();
    let mut after_double_dash = false;
    for &arg in args {
        if !after_double_dash && arg == "--" {
            after_double_dash = true;
            continue;
        }
        if !after_double_dash && arg == "--json" {
            json = true;
            continue;
        }
        if !after_double_dash && arg.starts_with('-') {
            return Err(format!("unknown option: {arg}"));
        }
        positional.push(arg);
    }
    Ok((json, positional))
}

fn run_path(args: &[&str], path_override: Option<&Path>) -> Result<String, String> {
    let (json, positional) = split_json_flag(args)?;
    if !positional.is_empty() {
        return Err(format!("unexpected argument: {}", positional[0]));
    }
    let path = resolve_config_path(path_override)?;
    if json {
        to_json_pretty(&PathOutput {
            path: path.display().to_string(),
            exists: path.exists(),
        })
    } else {
        Ok(path.display().to_string())
    }
}

fn run_show(args: &[&str], path_override: Option<&Path>) -> Result<String, String> {
    let (json, positional) = split_json_flag(args)?;
    if !positional.is_empty() {
        return Err(format!("unexpected argument: {}", positional[0]));
    }
    let path = resolve_config_path(path_override)?;
    let (doc, exists) = ConfigDoc::load_or_default(&path)?;
    if json {
        let settings = doc.effective_settings();
        let rules = doc.rule_views();
        to_json_pretty(&ShowOutput {
            path: path.display().to_string(),
            exists,
            version: doc.version,
            settings,
            labs: doc.labs,
            rules,
            workspaces: doc.workspaces,
            displays: doc.displays,
            shortcuts: doc.shortcuts,
        })
    } else {
        doc.render_effective_toml()
            .map(|rendered| rendered.trim_end().to_owned())
    }
}

fn run_get(args: &[&str], path_override: Option<&Path>) -> Result<String, String> {
    let (json, positional) = split_json_flag(args)?;
    let [raw_key] = positional.as_slice() else {
        return Err("usage: spaceman config get <key> [--json]".into());
    };
    let key = SettingKey::parse(raw_key)?;
    let path = resolve_config_path(path_override)?;
    let (doc, _) = ConfigDoc::load_or_default(&path)?;
    let (text, value) = match key {
        SettingKey::Hud => {
            let hud = doc.settings.effective_hud();
            (hud.to_string(), serde_json::Value::Bool(hud))
        }
        SettingKey::HudStyle => {
            let style = doc.settings.effective_hud_style().as_str();
            (
                style.to_owned(),
                serde_json::Value::String(style.to_owned()),
            )
        }
        SettingKey::UpdateChannel => {
            let channel = doc.settings.effective_update_channel().as_str();
            (
                channel.to_owned(),
                serde_json::Value::String(channel.to_owned()),
            )
        }
        SettingKey::MouseFollowsFocus => {
            let mode = doc.settings.effective_mouse_follows_focus().as_str();
            (mode.to_owned(), serde_json::Value::String(mode.to_owned()))
        }
        SettingKey::Spacing(key) => {
            let value = key.get(&doc.settings.effective_spacing());
            (value.to_string(), serde_json::Value::from(value))
        }
    };
    if json {
        to_json_pretty(&SettingOutput {
            path: None,
            key: key.as_str(),
            value,
        })
    } else {
        Ok(text)
    }
}

enum SettingValue {
    Hud(bool),
    HudStyle(HudStyle),
    UpdateChannel(UpdateChannel),
    MouseFollowsFocus(MouseFollowsFocus),
    Spacing(SpacingKey, i64),
}

impl SettingValue {
    fn parse(key: SettingKey, raw_value: &str) -> Result<Self, String> {
        match key {
            SettingKey::Hud => match raw_value {
                "true" => Ok(Self::Hud(true)),
                "false" => Ok(Self::Hud(false)),
                _ => Err(format!(
                    "invalid value for hud: expected 'true' or 'false', got {raw_value:?}"
                )),
            },
            SettingKey::HudStyle => HudStyle::parse(raw_value)
                .map(Self::HudStyle)
                .ok_or_else(|| {
                    format!(
                        "invalid value for hud_style: expected 'icons' or 'layouts', got {raw_value:?}"
                    )
                }),
            SettingKey::UpdateChannel => UpdateChannel::parse(raw_value)
                .map(Self::UpdateChannel)
                .ok_or_else(|| {
                    format!(
                        "invalid value for update_channel: expected 'stable' or 'dev', got {raw_value:?}"
                    )
                }),
            SettingKey::MouseFollowsFocus => MouseFollowsFocus::parse(raw_value)
                .map(Self::MouseFollowsFocus)
                .ok_or_else(|| {
                    format!(
                        "invalid value for mouse_follows_focus: expected 'off', 'display', or 'window', got {raw_value:?}"
                    )
                }),
            SettingKey::Spacing(spacing_key) => {
                let parsed = raw_value.parse::<i64>().map_err(|_| {
                    format!(
                        "invalid value for {}: expected a whole number of points, got {raw_value:?}",
                        spacing_key.path()
                    )
                })?;
                spacing_key.check(parsed)?;
                Ok(Self::Spacing(spacing_key, parsed))
            }
        }
    }
}

fn run_set(args: &[&str], path_override: Option<&Path>) -> Result<String, String> {
    let (json, positional) = split_json_flag(args)?;
    let [raw_key, raw_value] = positional.as_slice() else {
        return Err("usage: spaceman config set <key> <value> [--json]".into());
    };
    let key = SettingKey::parse(raw_key)?;
    let parsed_value = SettingValue::parse(key, raw_value)?;
    let path = resolve_config_path(path_override)?;

    let (text, value) = ConfigDoc::edit_locked(&path, |doc| {
        let output = match parsed_value {
            SettingValue::Hud(parsed) => {
                doc.settings.hud = Some(parsed);
                (
                    format!("set hud = {parsed}"),
                    serde_json::Value::Bool(parsed),
                )
            }
            SettingValue::HudStyle(parsed) => {
                doc.settings.hud_style = Some(parsed);
                (
                    format!("set hud_style = {}", parsed.as_str()),
                    serde_json::Value::String(parsed.as_str().to_owned()),
                )
            }
            SettingValue::UpdateChannel(parsed) => {
                doc.settings.update_channel = Some(parsed);
                (
                    format!("set update_channel = {}", parsed.as_str()),
                    serde_json::Value::String(parsed.as_str().to_owned()),
                )
            }
            SettingValue::MouseFollowsFocus(parsed) => {
                doc.settings.mouse_follows_focus = Some(parsed);
                (
                    format!("set mouse_follows_focus = {}", parsed.as_str()),
                    serde_json::Value::String(parsed.as_str().to_owned()),
                )
            }
            SettingValue::Spacing(spacing_key, parsed) => {
                let value = doc.settings.set_spacing(spacing_key, parsed)?;
                (
                    format!("set {} = {value}", spacing_key.path()),
                    serde_json::Value::from(value),
                )
            }
        };
        Ok((output, true))
    })?;
    if json {
        to_json_pretty(&SettingOutput {
            path: Some(path.display().to_string()),
            key: key.as_str(),
            value,
        })
    } else {
        Ok(text)
    }
}

fn run_check(args: &[&str], path_override: Option<&Path>) -> Result<String, String> {
    let (json, positional) = split_json_flag(args)?;
    let (path, explicit): (PathBuf, bool) = match positional.as_slice() {
        [] => (resolve_config_path(path_override)?, false),
        [target] => (PathBuf::from(target), true),
        _ => return Err("usage: spaceman config check [path] [--json]".into()),
    };

    let exists = if explicit {
        load_path(&path)?;
        true
    } else {
        let (_, exists) = ConfigDoc::load_or_default(&path)?;
        exists
    };

    if json {
        to_json_pretty(&CheckOutput {
            path: path.display().to_string(),
            exists,
            valid: true,
        })
    } else if exists {
        Ok("configuration is valid".into())
    } else {
        Ok(format!(
            "configuration is valid (using defaults; no file at {})",
            path.display()
        ))
    }
}
