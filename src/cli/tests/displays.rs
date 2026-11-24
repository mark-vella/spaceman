use serde_json::{Value, json};
use spaceman::config::Config;

use super::TempConfig;

const SOURCE: &str = r#"# Mine.
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

#[test]
fn config_show_lists_display_overrides_in_file_order() {
    let temp = TempConfig::new("displays-show");
    temp.write(SOURCE);

    let show: Value = serde_json::from_str(
        temp.run(&["config", "show", "--json"])
            .stdout
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        show["displays"],
        json!([
            {"monitor": "Built-in Retina Display", "single_window_max_width": 0},
            {"monitor": "M34WQ", "single_window_max_width": 2670},
        ])
    );

    let shown = temp.run(&["config", "show"]).stdout.unwrap();
    assert!(
        shown.contains(
            "[[displays]]\nmonitor = \"Built-in Retina Display\"\nsingle_window_max_width = 0\n\n\
             [[displays]]\nmonitor = \"M34WQ\"\nsingle_window_max_width = 2670"
        ),
        "{shown}"
    );

    let empty = TempConfig::new("displays-show-empty");
    empty.write("version = 1\n");
    let show: Value = serde_json::from_str(
        empty
            .run(&["config", "show", "--json"])
            .stdout
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(show["displays"], json!([]));
}

#[test]
fn cli_saves_keep_display_overrides_and_their_order() {
    let temp = TempConfig::new("displays-save");
    temp.write(SOURCE);

    for args in [
        &["config", "set", "gaps.inner", "8"][..],
        &["workspaces", "assign", "2", "secondary"][..],
        &["rules", "add", "--app", "com.example.App", "--float"][..],
    ] {
        let outcome = temp.run(args);
        assert_eq!(outcome.exit_code, 0, "{args:?}: {:?}", outcome.stderr);
    }

    let saved = Config::parse(&temp.read()).unwrap();
    let monitors: Vec<_> = saved
        .display_overrides()
        .iter()
        .map(|entry| (entry.monitor.as_str(), entry.single_window_max_width))
        .collect();
    assert_eq!(monitors, [("Built-in Retina Display", 0), ("M34WQ", 2670)]);
    assert_eq!(saved.spacing().inner, 8);
}

#[test]
fn config_check_rejects_a_bad_display_override() {
    let temp = TempConfig::new("displays-check");
    temp.write(SOURCE);
    assert_eq!(temp.run(&["config", "check"]).exit_code, 0);

    temp.write(&SOURCE.replace("2670", "100"));
    let bad = temp.run(&["config", "check"]);
    assert_eq!(bad.exit_code, 1);
    assert!(
        bad.stderr.as_deref().unwrap().contains(
            "display \"M34WQ\" single_window_max_width must be 0 (off) or between 320 and 16384 px, got 100"
        ),
        "{:?}",
        bad.stderr
    );
}
