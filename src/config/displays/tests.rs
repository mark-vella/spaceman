use crate::{
    DisplaySelector, WorkspaceId,
    config::{Config, DisplayOverride, load_path, now, save_workspace_monitor_at},
};

const TWO_DISPLAYS: &str = r#"
version = 1
[settings]
single_window_max_width = 1680

[[displays]]
monitor = "Built-in Retina Display"
single_window_max_width = 0

[[displays]]
monitor = "M34WQ"
single_window_max_width = 2670
"#;

fn error(displays: &str) -> String {
    Config::parse(&format!("version = 1\n{displays}")).unwrap_err()
}

#[test]
fn parses_entries_in_file_order_with_every_selector_form() {
    let config = Config::parse(
        r#"
version = 1
[[displays]]
monitor = "main"
single_window_max_width = 1440
[[displays]]
monitor = "secondary"
single_window_max_width = 0
[[displays]]
monitor = "37D8832A-2D66-02CA-B9F7-8F30A301B230"
single_window_max_width = 320
[[displays]]
monitor = "M34WQ"
single_window_max_width = 16384
"#,
    )
    .unwrap();
    assert_eq!(
        config.display_overrides(),
        [
            DisplayOverride {
                monitor: DisplaySelector::Main,
                single_window_max_width: 1440,
            },
            DisplayOverride {
                monitor: DisplaySelector::Secondary,
                single_window_max_width: 0,
            },
            DisplayOverride {
                monitor: DisplaySelector::Preferred("37D8832A-2D66-02CA-B9F7-8F30A301B230".into()),
                single_window_max_width: 320,
            },
            DisplayOverride {
                monitor: DisplaySelector::Preferred("M34WQ".into()),
                single_window_max_width: 16384,
            },
        ]
    );
}

#[test]
fn rejects_bad_entries_with_clear_messages() {
    for (displays, message) in [
        (
            "[[displays]]\nmonitor = \"\"\nsingle_window_max_width = 0",
            "display monitor cannot be empty",
        ),
        (
            "[[displays]]\nmonitor = \"  \"\nsingle_window_max_width = 0",
            "display monitor cannot be empty",
        ),
        (
            "[[displays]]\nmonitor = \"M34WQ\"",
            "display \"M34WQ\" overrides nothing; set single_window_max_width",
        ),
        (
            "[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 0\n\
             [[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 1440",
            "display \"M34WQ\" is configured more than once",
        ),
        (
            "[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 100",
            "display \"M34WQ\" single_window_max_width must be 0 (off) or between 320 and 16384 px, got 100",
        ),
        (
            "[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 99999",
            "display \"M34WQ\" single_window_max_width must be 0 (off) or between 320 and 16384 px, got 99999",
        ),
        (
            "[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = -1",
            "display \"M34WQ\" single_window_max_width must be 0 (off) or between 320 and 16384 px, got -1",
        ),
    ] {
        assert_eq!(error(displays), message, "{displays}");
    }
    for (displays, message) in [
        (
            "[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 0\ngaps = 4",
            "unknown field `gaps`",
        ),
        (
            "[[displays]]\nsingle_window_max_width = 0",
            "missing field `monitor`",
        ),
        (
            "[[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 1.5",
            "invalid type",
        ),
    ] {
        let error = error(displays);
        assert!(error.contains(message), "{displays}: {error}");
    }
}

#[test]
fn without_displays_every_display_uses_the_global_spacing() {
    let config =
        Config::parse("version = 1\n[settings]\nsingle_window_max_width = 1680\n").unwrap();
    assert!(config.display_overrides().is_empty());
    assert_eq!(config.spacing_on(|_| true).single_window_max_width, 1680);
    assert_eq!(
        Config::default()
            .spacing_on(|_| true)
            .single_window_max_width,
        0
    );
}

#[test]
fn the_first_entry_that_selects_a_display_wins() {
    let config = Config::parse(
        r#"
version = 1
[settings]
single_window_max_width = 1680
[settings.gaps]
inner = 8
[[displays]]
monitor = "M34WQ"
single_window_max_width = 2670
[[displays]]
monitor = "secondary"
single_window_max_width = 1440
"#,
    )
    .unwrap();
    let ultrawide_as_secondary = |selector: &DisplaySelector| {
        matches!(selector, DisplaySelector::Secondary)
            || *selector == DisplaySelector::Preferred("M34WQ".into())
    };
    let spacing = config.spacing_on(ultrawide_as_secondary);
    assert_eq!((spacing.single_window_max_width, spacing.inner), (2670, 8));
    assert_eq!(
        config
            .display_override(ultrawide_as_secondary)
            .map(|(index, _)| index),
        Some(0)
    );
    let other_secondary =
        |selector: &DisplaySelector| matches!(selector, DisplaySelector::Secondary);
    assert_eq!(
        config.spacing_on(other_secondary).single_window_max_width,
        1440
    );
    assert_eq!(config.spacing_on(|_| false).single_window_max_width, 1680);
}

#[test]
fn an_override_of_zero_turns_the_cap_off_on_its_display() {
    let config = Config::parse(TWO_DISPLAYS).unwrap();
    let built_in = |selector: &DisplaySelector| selector.as_str() == "Built-in Retina Display";
    assert_eq!(config.spacing().single_window_max_width, 1680);
    assert_eq!(config.spacing_on(built_in).single_window_max_width, 0);
}

#[test]
fn saving_a_workspace_monitor_keeps_displays_in_order() {
    let directory = std::env::temp_dir().join(format!(
        "spaceman-displays-{}-{}",
        std::process::id(),
        now().as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    std::fs::write(&path, TWO_DISPLAYS).unwrap();

    save_workspace_monitor_at(&path, WorkspaceId::TWO, &DisplaySelector::Secondary).unwrap();

    let saved = load_path(&path).unwrap();
    let monitors: Vec<_> = saved
        .display_overrides()
        .iter()
        .map(|entry| {
            (
                entry.monitor.as_str().to_owned(),
                entry.single_window_max_width,
            )
        })
        .collect();
    assert_eq!(
        monitors,
        [
            ("Built-in Retina Display".to_owned(), 0),
            ("M34WQ".to_owned(), 2670)
        ]
    );
    std::fs::remove_dir_all(directory).unwrap();
}
