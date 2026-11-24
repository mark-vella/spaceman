use super::*;

/// A 1512 × 945 built-in main display and a 3440 × 1440 ultrawide to its right.
fn laptop_and_ultrawide() -> macos::DisplayOutputs {
    macos::DisplayOutputs::new(vec![
        macos::DisplayOutput {
            id: "built-in-id".into(),
            name: "Built-in Retina Display".into(),
            frame: crate::Rect {
                x: 0,
                y: 0,
                width: 1512,
                height: 945,
            },
            is_main: true,
        },
        macos::DisplayOutput {
            id: "ultrawide-id".into(),
            name: "M34WQ".into(),
            frame: crate::Rect {
                x: 1512,
                y: 0,
                width: 3440,
                height: 1440,
            },
            is_main: false,
        },
    ])
    .unwrap()
}

/// 1200 everywhere, except the built-in display fills and the ultrawide caps at 2670.
fn per_display_config() -> Config {
    Config::parse(
        r#"
version = 1
[settings]
single_window_max_width = 1200

[[displays]]
monitor = "Built-in Retina Display"
single_window_max_width = 0

[[displays]]
monitor = "M34WQ"
single_window_max_width = 2670
"#,
    )
    .unwrap()
}

const BUILT_IN_FILLED: crate::Rect = crate::Rect {
    x: 0,
    y: 0,
    width: 1512,
    height: 945,
};

/// 2670 wide, centered in the ultrawide's 3440: 385 in from its left edge at 1512.
const ULTRAWIDE_CAPPED: crate::Rect = crate::Rect {
    x: 1897,
    y: 0,
    width: 2670,
    height: 1440,
};

fn observing(windows: &[&crate::PhysicalWindowId]) -> crate::NativeObservation {
    crate::NativeObservation {
        cycle: 1,
        mouse_down: false,
        applications: vec![crate::AppObservation::Complete {
            app: crate::ObservedApplication {
                id: windows[0].app.clone(),
                bundle_id: "com.example.Test".into(),
            },
            focused: Some(windows[0].clone()),
            windows: windows
                .iter()
                .map(|id| crate::ObservedWindow {
                    id: (*id).clone(),
                    frame: frame(0),
                    resizable: true,
                    title: None,
                })
                .collect(),
        }],
    }
}

fn run(
    runtime: &mut WorkspaceRuntime<FakeBackend>,
    action: WorkerAction,
    outputs: macos::DisplayOutputs,
) {
    let result = runtime.execute(
        WorkerCommand::Action(action, outputs),
        &AtomicBool::new(false),
    );
    let Some(WorkerResult::Status {
        result: Ok(status), ..
    }) = result
    else {
        panic!("action failed");
    };
    assert_eq!(status.failed, 0);
}

#[test]
fn each_display_gives_its_lone_window_its_own_width() {
    let (laptop, wide, wide_second) = (window(10), window(11), window(12));
    let mut backend = FakeBackend {
        observation: Some(observing(&[&laptop, &wide, &wide_second])),
        ..FakeBackend::default()
    };
    backend.frames.insert(laptop.clone(), frame(0));
    for window in [&wide, &wide_second] {
        backend.frames.insert(window.clone(), frame(2000));
    }
    let mut runtime = runtime_with_config(backend, per_display_config());
    bind(&mut runtime, WorkspaceId::ONE, &laptop);
    bind(&mut runtime, WorkspaceId::SIX, &wide);
    bind(&mut runtime, WorkspaceId::SIX, &wide_second);
    runtime
        .workspaces
        .set_fullscreen_layout(WorkspaceId::SIX, true);

    run(&mut runtime, WorkerAction::Retile, laptop_and_ultrawide());

    assert_eq!(runtime.backend.frames[&laptop], BUILT_IN_FILLED);
    assert_eq!(runtime.backend.frames[&wide], ULTRAWIDE_CAPPED);
    assert_eq!(runtime.backend.frames[&wide_second], ULTRAWIDE_CAPPED);
}

#[test]
fn a_workspace_moved_to_another_display_takes_that_displays_width() {
    let lone = window(10);
    let mut backend = FakeBackend {
        focused: Some(lone.clone()),
        ..FakeBackend::default()
    };
    backend.frames.insert(lone.clone(), frame(0));
    let mut runtime = runtime_with_config(backend, per_display_config());
    bind(&mut runtime, WorkspaceId::ONE, &lone);

    run(
        &mut runtime,
        WorkerAction::MoveWorkspaceToDisplay(DisplayTarget::Selector(DisplaySelector::Secondary)),
        laptop_and_ultrawide(),
    );
    assert_eq!(runtime.backend.frames[&lone], ULTRAWIDE_CAPPED);

    run(
        &mut runtime,
        WorkerAction::MoveWorkspaceToDisplay(DisplayTarget::Selector(DisplaySelector::Main)),
        laptop_and_ultrawide(),
    );
    assert_eq!(runtime.backend.frames[&lone], BUILT_IN_FILLED);
}

#[test]
fn entries_for_displays_not_connected_change_nothing() {
    let lone = window(10);
    let mut backend = FakeBackend {
        observation: Some(observing(&[&lone])),
        ..FakeBackend::default()
    };
    backend.frames.insert(lone.clone(), frame(0));
    let config = Config::parse(
        r#"
version = 1
[settings]
single_window_max_width = 1200

[[displays]]
monitor = "secondary"
single_window_max_width = 0

[[displays]]
monitor = "M34WQ"
single_window_max_width = 2670
"#,
    )
    .unwrap();
    let mut runtime = runtime_with_config(backend, config);
    bind(&mut runtime, WorkspaceId::ONE, &lone);
    let laptop_alone =
        macos::DisplayOutputs::new(vec![laptop_and_ultrawide().main().clone()]).unwrap();

    run(&mut runtime, WorkerAction::Retile, laptop_alone);

    // The global 1200, centered in the built-in display's 1512.
    assert_eq!(
        runtime.backend.frames[&lone],
        crate::Rect {
            x: 156,
            y: 0,
            width: 1200,
            height: 945,
        }
    );
}
