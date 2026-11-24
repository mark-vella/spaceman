//! Settings pane registry and playground scenes.

mod general;
mod layout;
pub mod shortcuts;
pub(crate) mod workspaces;

use rune::{El, Icon};
use spaceman::app::{UiLaunchAtLogin, UiUpdateStatus};

use crate::{
    Ui, activity,
    controls::Paint,
    map,
    playground::{Scene, reset_fixture},
};

static GENERAL_ICON: Icon = Icon {
    name: "spaceman-general",
    view: 16.0,
    body: include_str!("../assets/general.svg"),
};
static LAYOUT_ICON: Icon = Icon {
    name: "spaceman-layout",
    view: 16.0,
    body: include_str!("../assets/layout.svg"),
};
static RULES_ICON: Icon = Icon {
    name: "spaceman-rules",
    view: 16.0,
    body: include_str!("../assets/rules.svg"),
};
static WORKSPACES_ICON: Icon = Icon {
    name: "spaceman-workspaces",
    view: 16.0,
    body: include_str!("../assets/workspaces.svg"),
};
static ACTIVITY_ICON: Icon = Icon {
    name: "spaceman-activity",
    view: 24.0,
    body: include_str!("../assets/activity.svg"),
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    General,
    Layout,
    Rules,
    Workspaces,
    Shortcuts,
    Activity,
}

impl Pane {
    pub const ALL: [Pane; 6] = [
        Pane::General,
        Pane::Layout,
        Pane::Rules,
        Pane::Workspaces,
        Pane::Shortcuts,
        Pane::Activity,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Pane::General => "general",
            Pane::Layout => "layout",
            Pane::Rules => "rules",
            Pane::Workspaces => "workspaces",
            Pane::Shortcuts => "shortcuts",
            Pane::Activity => "activity",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Pane::General => "General",
            Pane::Layout => "Layout",
            Pane::Rules => "Rules",
            Pane::Workspaces => "Workspaces",
            Pane::Shortcuts => "Shortcuts",
            Pane::Activity => "Activity",
        }
    }

    pub const fn icon(self) -> &'static Icon {
        match self {
            Pane::General => &GENERAL_ICON,
            Pane::Layout => &LAYOUT_ICON,
            Pane::Rules => &RULES_ICON,
            Pane::Workspaces => &WORKSPACES_ICON,
            Pane::Shortcuts => &rune_theme_opencode::icons::KEYBOARD,
            Pane::Activity => &ACTIVITY_ICON,
        }
    }

    pub fn render(self, ui: &Ui, p: Paint, layer: &mut Vec<El>) -> El {
        match self {
            Pane::General => general::pane(ui, p),
            Pane::Layout => layout::pane(ui, p),
            Pane::Rules => map::pane(ui, p, layer),
            Pane::Workspaces => workspaces::pane(ui, p),
            Pane::Shortcuts => shortcuts::pane(ui, p),
            Pane::Activity => activity::pane(ui, p),
        }
    }

    /// Maps `⌘1`..`⌘N` in sidebar order.
    pub fn from_shortcut(digit: char) -> Option<Pane> {
        let index = digit.to_digit(10)?.checked_sub(1)? as usize;
        Self::ALL.get(index).copied()
    }

    /// Maps a `spaceman://` deep-link pane to a registered Settings pane.
    ///
    /// Returns `None` for panes not registered in this build (such as `Labs`
    /// or `WhatsNew`) so the caller can fall back cleanly while preserving
    /// `highlight_lab` / `highlight_feature`.
    pub const fn from_deep_link(pane: spaceman::deep_link::DeepLinkPane) -> Option<Pane> {
        match pane {
            spaceman::deep_link::DeepLinkPane::General => Some(Pane::General),
            spaceman::deep_link::DeepLinkPane::Layout => Some(Pane::Layout),
            spaceman::deep_link::DeepLinkPane::Rules => Some(Pane::Rules),
            spaceman::deep_link::DeepLinkPane::Workspaces => Some(Pane::Workspaces),
            spaceman::deep_link::DeepLinkPane::Shortcuts => Some(Pane::Shortcuts),
            spaceman::deep_link::DeepLinkPane::Activity => Some(Pane::Activity),
            spaceman::deep_link::DeepLinkPane::Labs
            | spaceman::deep_link::DeepLinkPane::WhatsNew => None,
        }
    }
}

pub(crate) const SCENES: &[Scene] = &[
    ("general", |ui| reset_fixture(ui, Pane::General)),
    ("general-setup", |ui| {
        reset_fixture(ui, Pane::General);
        if let Some(agent) = &mut ui.agent {
            agent.accessibility = false;
            agent.automatic = false;
            agent.launch_at_login = UiLaunchAtLogin::Off;
        }
    }),
    ("general-update-ready", |ui| {
        reset_fixture(ui, Pane::General);
        if let Some(agent) = &mut ui.agent {
            agent.update = UiUpdateStatus::Ready {
                version: "0.3.3".into(),
            };
        }
    }),
    ("layout", |ui| reset_fixture(ui, Pane::Layout)),
    // Gaps, uneven edges for a custom bar, and a width cap that brings in the ultrawide.
    ("layout-spaced", |ui| {
        reset_fixture(ui, Pane::Layout);
        ui.edit(|doc| {
            doc.set_spacing(spaceman::spacing::Spacing {
                inner: 10,
                top: 32,
                bottom: 8,
                left: 8,
                right: 8,
                single_window_max_width: 1680,
            });
        });
    }),
    // A laptop and an ultrawide: the width picker edits every display at once.
    ("layout-displays", |ui| {
        reset_fixture(ui, Pane::Layout);
        ui.displays = workspaces::fixture_laptop_and_ultrawide();
        ui.edit(|doc| {
            doc.set_spacing(spaceman::spacing::Spacing {
                single_window_max_width: 1680,
                ..spaceman::spacing::Spacing::NONE
            })
        });
    }),
    // The ultrawide picked, holding its own width while the laptop fills.
    ("layout-display-override", |ui| {
        reset_fixture(ui, Pane::Layout);
        ui.displays = workspaces::fixture_laptop_and_ultrawide();
        ui.edit(|doc| {
            doc.set_spacing(spaceman::spacing::Spacing {
                single_window_max_width: 1680,
                ..spaceman::spacing::Spacing::NONE
            })
        });
        let (laptop, ultrawide) = (ui.displays[0].id.clone(), ui.displays[1].id.clone());
        ui.set_display_width(&laptop, Some(0));
        ui.set_display_width(&ultrawide, Some(1920));
        ui.layout_display = Some(ultrawide);
    }),
    ("workspaces", |ui| reset_fixture(ui, Pane::Workspaces)),
    // A third monitor: workspace 9 assigned to it, its segment in every row's picker.
    ("workspaces-three-displays", |ui| {
        reset_fixture(ui, Pane::Workspaces);
        ui.displays = workspaces::fixture_three();
        let nine = spaceman::WorkspaceId::new(9).unwrap_or(spaceman::WorkspaceId::SIX);
        ui.edit(|doc| doc.set_monitor(nine, Some(workspaces::THIRD_FIXTURE_ID)));
    }),
    // A fourth monitor: the picker numbers the last two so it stays one readable row.
    ("workspaces-four-displays", |ui| {
        reset_fixture(ui, Pane::Workspaces);
        ui.displays = workspaces::fixture_quad();
    }),
    // The third monitor unplugged: its saved assignment keeps its own card.
    ("workspaces-unplugged", |ui| {
        reset_fixture(ui, Pane::Workspaces);
        let nine = spaceman::WorkspaceId::new(9).unwrap_or(spaceman::WorkspaceId::SIX);
        ui.edit(|doc| doc.set_monitor(nine, Some(workspaces::THIRD_FIXTURE_ID)));
    }),
    ("shortcuts", |ui| reset_fixture(ui, Pane::Shortcuts)),
    ("shortcuts-recording", |ui| {
        reset_fixture(ui, Pane::Shortcuts);
        shortcuts::fixture_recording(ui, None);
    }),
    ("shortcuts-refused", |ui| {
        reset_fixture(ui, Pane::Shortcuts);
        shortcuts::fixture_recording(ui, Some("Used by Next window"));
    }),
    ("shortcuts-custom", |ui| {
        reset_fixture(ui, Pane::Shortcuts);
        shortcuts::fixture_custom(ui);
    }),
];
