use paimon_probe::route_execution::{ExecutionScope, RouteTask};
use paimon_probe::{
    movement::Conditions,
    navigation::NavigationState,
    route_execution::{RouteCommand, RouteFrame},
};

const ORIGINAL: &[u8] = include_bytes!("fixtures/bettergi/route-1.json");

#[test]
fn consecutive_teleport_points_each_require_their_own_map_loading_and_arrival_evidence() {
    let source=br#"{"info":{},"positions":[{"id":9,"x":267.96,"y":-665.16,"type":"teleport"},{"id":1,"x":267.96,"y":-665.16,"type":"teleport"}]}"#;
    let mut task = RouteTask::prepare(
        "two-teleports.json",
        source,
        ExecutionScope::WholeRoute,
        1000,
    )
    .unwrap();
    observe(&mut task, 1000, "world");
    let c = observe(&mut task, 1200, "world");
    release_tap(&mut task, c, 1400);
    observe(&mut task, 1600, "map-zoom-3");
    let c = observe(&mut task, 1800, "map-zoom-3");
    release_tap(&mut task, c, 2000);
    let c = observe(&mut task, 2200, "map-selected");
    release_tap(&mut task, c, 2400);
    let c = observe(&mut task, 2600, "map-detail");
    release_tap(&mut task, c, 2800);
    observe(&mut task, 3000, "loading");
    observe(&mut task, 3200, "arrived");
    observe(&mut task, 3400, "arrived");
    assert_eq!(task.report().completed_points.len(), 1);
    assert!(!task.report().original_route_passed);
    observe(&mut task, 3600, "arrived");
    let c = observe(&mut task, 3800, "arrived");
    release_tap(&mut task, c, 4000);
    observe(&mut task, 4200, "map-zoom-3");
    let c = observe(&mut task, 4400, "map-zoom-3");
    release_tap(&mut task, c, 4600);
    let c = observe(&mut task, 4800, "map-selected");
    release_tap(&mut task, c, 5000);
    let c = observe(&mut task, 5200, "map-detail");
    release_tap(&mut task, c, 5400);
    observe(&mut task, 5600, "loading");
    observe(&mut task, 5800, "arrived");
    observe(&mut task, 6000, "arrived");
    assert!(task.report().original_route_passed);
    assert_eq!(
        task.report()
            .completed_points
            .iter()
            .map(|p| p.original_id.as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![9, 1]
    );
    assert_eq!(task.report().command_count, 8);
}

#[test]
fn the_complete_real_phone_teleport_run_replays_original_bytes_pixels_and_input_receipts() {
    use serde_json_lenient::Value;
    let recorded: Value = serde_json_lenient::from_slice(include_bytes!(
        "../../docs/validation/reports/issue-19/teleport.json"
    ))
    .unwrap();
    let history = recorded["history"].as_array().unwrap();
    let mut task = RouteTask::prepare(
        "original.json",
        ORIGINAL,
        ExecutionScope::FirstTeleport,
        history[0]["observedAt"].as_u64().unwrap(),
    )
    .unwrap();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/teleport/phone-run");
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    files.sort();
    let mut images = files.iter();
    let mut pending = None;
    for entry in &history[1..] {
        let now = entry["observedAt"].as_u64().unwrap();
        if let Some(serial) = pending.take() {
            task.input_finished(serial, now, true, true);
            continue;
        }
        let file = images.next().unwrap();
        let stem = file.file_stem().unwrap().to_str().unwrap();
        let at = stem.parse().unwrap();
        let mini = image::open(file).unwrap().to_rgb8();
        let screen = image::open(directory.join(format!("{stem}-screen.jpg")))
            .unwrap()
            .to_rgb8();
        let command = task.step(
            now,
            Conditions::Ready,
            Some(RouteFrame {
                captured_at: at,
                screen: Some(screen.as_raw()),
                minimap: mini.as_raw(),
            }),
        );
        let actual = serde_json_lenient::to_value(command).unwrap();
        assert_eq!(
            actual["kind"],
            entry["command"]["kind"],
            "frame {stem}: {}",
            task.report().reason
        );
        if let RouteCommand::Tap { serial, x, y } = command {
            assert_eq!(serial, entry["command"]["serial"].as_u64().unwrap());
            assert!(
                (x as f64 - entry["command"]["x"].as_f64().unwrap())
                    .hypot(y as f64 - entry["command"]["y"].as_f64().unwrap())
                    <= 6.0
            );
            pending = Some(serial);
        }
    }
    assert!(images.next().is_none());
    assert_eq!(task.report().state, NavigationState::Completed);
    assert!(task.report().teleport_validation_passed && !task.report().original_route_passed);
    assert_eq!(task.report().command_count, 4);
    let initial = task.report().initial_position.unwrap();
    let final_position = task.report().position.unwrap();
    assert!((initial.x - final_position.x).hypot(initial.y - final_position.y) > 150.0);
    assert!(task.report().completed_points[0].distance <= 8.0);
}

#[test]
fn map_controls_are_recognized_over_a_different_real_viewport_and_runtime_jpeg() {
    let world = image::load_from_memory(include_bytes!("fixtures/teleport/runtime-world-127.jpg"))
        .unwrap()
        .to_rgb8();
    let mini = image::load_from_memory(include_bytes!(
        "fixtures/teleport/runtime-world-127-mini.png"
    ))
    .unwrap()
    .to_rgb8();
    let map = image::load_from_memory(include_bytes!("fixtures/teleport/runtime-map-127.jpg"))
        .unwrap()
        .to_rgb8();
    let map_mini =
        image::load_from_memory(include_bytes!("fixtures/teleport/runtime-map-127-mini.png"))
            .unwrap()
            .to_rgb8();
    let mut task = RouteTask::prepare(
        "original.json",
        ORIGINAL,
        ExecutionScope::FirstTeleport,
        1000,
    )
    .unwrap();
    for at in [1000, 1200] {
        let c = task.step(
            at,
            Conditions::Ready,
            Some(RouteFrame {
                captured_at: at,
                screen: Some(world.as_raw()),
                minimap: mini.as_raw(),
            }),
        );
        if at == 1200 {
            release_tap(&mut task, c, 1400);
        }
    }
    let mut c = task.step(
        1600,
        Conditions::Ready,
        Some(RouteFrame {
            captured_at: 1600,
            screen: Some(map.as_raw()),
            minimap: map_mini.as_raw(),
        }),
    );
    if c == RouteCommand::Observe {
        c = task.step(
            1800,
            Conditions::Ready,
            Some(RouteFrame {
                captured_at: 1800,
                screen: Some(map.as_raw()),
                minimap: map_mini.as_raw(),
            }),
        );
    }
    let RouteCommand::Tap { x, y, .. } = c else {
        panic!("{c:?}: {}", task.report().reason)
    };
    assert!(
        ((x as f64 - 154.0).hypot(y as f64 - 434.0) < 16.0)
            || ((x as f64 - 1305.0).hypot(y as f64 - 670.0) < 24.0),
        "Expected an observed zoom control or atlas-backed anchor, got {x},{y}"
    );
}

fn observe(task: &mut RouteTask, at: u64, name: &str) -> RouteCommand {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/teleport");
    let screen = image::open(base.join(format!("{name}.png")))
        .unwrap()
        .to_rgb8();
    let mini = image::open(base.join(format!("{name}-mini.png")))
        .unwrap()
        .to_rgb8();
    task.step(
        at,
        Conditions::Ready,
        Some(RouteFrame {
            captured_at: at,
            screen: Some(screen.as_raw()),
            minimap: mini.as_raw(),
        }),
    )
}
fn release_tap(task: &mut RouteTask, command: RouteCommand, at: u64) -> (u32, u32) {
    let RouteCommand::Tap { serial, x, y } = command else {
        panic!("{command:?}: {}", task.report().reason)
    };
    task.input_finished(serial, at, true, true);
    (x, y)
}

fn awaiting_loading() -> RouteTask {
    let mut task = RouteTask::prepare(
        "original.json",
        ORIGINAL,
        ExecutionScope::FirstTeleport,
        1000,
    )
    .unwrap();
    observe(&mut task, 1000, "world");
    let c = observe(&mut task, 1200, "world");
    release_tap(&mut task, c, 1400);
    observe(&mut task, 1600, "map-zoom-3");
    let c = observe(&mut task, 1800, "map-zoom-3");
    release_tap(&mut task, c, 2000);
    let c = observe(&mut task, 2200, "map-selected");
    release_tap(&mut task, c, 2400);
    let c = observe(&mut task, 2600, "map-detail");
    release_tap(&mut task, c, 2800);
    task
}

#[test]
fn the_real_dark_gray_loading_page_is_transition_evidence_but_still_requires_relocalization() {
    let mut task = awaiting_loading();
    let screen = image::load_from_memory(include_bytes!("fixtures/teleport/runtime-loading.jpg"))
        .unwrap()
        .to_rgb8();
    let mini =
        image::load_from_memory(include_bytes!("fixtures/teleport/runtime-loading-mini.png"))
            .unwrap()
            .to_rgb8();
    assert_eq!(
        task.step(
            3000,
            Conditions::Ready,
            Some(RouteFrame {
                captured_at: 3000,
                screen: Some(screen.as_raw()),
                minimap: mini.as_raw()
            })
        ),
        RouteCommand::Observe
    );
    assert!(task.report().loading_observed);
    assert!(!task.report().teleport_validation_passed && task.report().position.is_none());
    observe(&mut task, 3200, "arrived");
    observe(&mut task, 3400, "arrived");
    assert!(task.report().teleport_validation_passed);
}

#[test]
fn a_missing_loading_transition_or_a_real_wrong_arrival_cannot_pass_the_teleport() {
    let mut task = awaiting_loading();
    for at in [3000, 3200, 3400] {
        assert_eq!(observe(&mut task, at, "arrived"), RouteCommand::Observe);
    }
    assert!(!task.report().loading_observed && task.report().completed_points.is_empty());
    assert_eq!(
        task.step(46_001, Conditions::Ready, None),
        RouteCommand::End
    );
    assert_eq!(task.report().state, NavigationState::Failed);
    let mut task = awaiting_loading();
    observe(&mut task, 3000, "loading");
    for at in [3200, 3400, 3600] {
        observe(&mut task, at, "world");
    }
    assert_eq!(
        task.report().state,
        NavigationState::Failed,
        "{}",
        task.report().reason
    );
    assert!(task.report().reason.contains("位置与原传送目标不符"));
    assert!(task.report().completed_points.is_empty() && !task.report().teleport_validation_passed);
}

#[test]
fn every_interruption_and_late_receipt_ends_a_pending_map_input_without_restarting_it() {
    for condition in [
        Conditions::UserStopped,
        Conditions::WrongForeground,
        Conditions::PermissionLost,
        Conditions::ScreenLost,
        Conditions::DisplayChanged,
        Conditions::Locked,
    ] {
        let mut task = RouteTask::prepare(
            "original.json",
            ORIGINAL,
            ExecutionScope::FirstTeleport,
            1000,
        )
        .unwrap();
        observe(&mut task, 1000, "world");
        let RouteCommand::Tap { serial, .. } = observe(&mut task, 1200, "world") else {
            panic!()
        };
        assert_eq!(task.step(1300, condition, None), RouteCommand::End);
        task.input_finished(serial, 1400, true, true);
        assert_eq!(observe(&mut task, 1600, "map-zoom-3"), RouteCommand::End);
        assert_eq!(task.report().command_count, 1);
        assert!(task.report().completed_points.is_empty() && task.report().position.is_none());
    }
    for (time, success, released) in [
        (2201, true, true),
        (1400, false, true),
        (1400, true, false),
        (1100, true, true),
    ] {
        let mut task = RouteTask::prepare(
            "original.json",
            ORIGINAL,
            ExecutionScope::FirstTeleport,
            1000,
        )
        .unwrap();
        observe(&mut task, 1000, "world");
        let RouteCommand::Tap { serial, .. } = observe(&mut task, 1200, "world") else {
            panic!()
        };
        task.input_finished(serial, time, success, released);
        assert_eq!(task.report().state, NavigationState::Failed);
        assert_eq!(observe(&mut task, 2400, "map-zoom-3"), RouteCommand::End);
    }
}

#[test]
fn teleport_needs_loading_and_two_fresh_relocalized_images_not_a_successful_tap_receipt() {
    let mut task = RouteTask::prepare(
        "original.json",
        ORIGINAL,
        ExecutionScope::FirstTeleport,
        1000,
    )
    .unwrap();
    observe(&mut task, 1000, "world");
    let command = observe(&mut task, 1200, "world");
    release_tap(&mut task, command, 1400);
    observe(&mut task, 1600, "map-zoom-3");
    let command = observe(&mut task, 1800, "map-zoom-3");
    release_tap(&mut task, command, 2000);
    let command = observe(&mut task, 2200, "map-selected");
    let choice = release_tap(&mut task, command, 2400);
    assert!((1600..1900).contains(&choice.0) && (630..710).contains(&choice.1));
    let command = observe(&mut task, 2600, "map-detail");
    let button = release_tap(&mut task, command, 2800);
    assert!((1900..2300).contains(&button.0) && (970..1080).contains(&button.1));
    assert!(!task.report().teleport_validation_passed && task.report().completed_points.is_empty());
    assert_eq!(observe(&mut task, 3000, "loading"), RouteCommand::Observe);
    assert_eq!(observe(&mut task, 3200, "arrived"), RouteCommand::Observe);
    assert!(!task.report().teleport_validation_passed);
    assert_eq!(observe(&mut task, 3400, "arrived"), RouteCommand::End);
    assert!(task.report().teleport_validation_passed && !task.report().original_route_passed);
    assert_eq!(task.report().current_index, 2);
    assert_eq!(task.report().completed_points[0].point_type, "teleport");
    assert!(task.report().completed_points[0].distance <= 8.0);
}

#[test]
fn the_original_route_hands_off_after_teleport_without_deleting_or_reordering_points() {
    let mut task =
        RouteTask::prepare("original.json", ORIGINAL, ExecutionScope::WholeRoute, 1000).unwrap();
    observe(&mut task, 1000, "world");
    let c = observe(&mut task, 1200, "world");
    release_tap(&mut task, c, 1400);
    observe(&mut task, 1600, "map-zoom-3");
    let c = observe(&mut task, 1800, "map-zoom-3");
    release_tap(&mut task, c, 2000);
    let c = observe(&mut task, 2200, "map-selected");
    release_tap(&mut task, c, 2400);
    let c = observe(&mut task, 2600, "map-detail");
    release_tap(&mut task, c, 2800);
    observe(&mut task, 3000, "loading");
    observe(&mut task, 3200, "arrived");
    observe(&mut task, 3400, "arrived");
    assert_eq!(task.report().current_index, 2);
    assert!(!task.report().original_route_passed);
    observe(&mut task, 3600, "arrived");
    observe(&mut task, 3800, "arrived");
    assert_eq!(task.report().current_index, 3);
    assert_eq!(
        task.report()
            .completed_points
            .iter()
            .map(|p| p.index)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        task.report().source_sha256,
        "1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2"
    );
}

#[test]
fn real_world_and_zoomed_map_find_the_anchor_in_screen_space_before_selecting_it() {
    let world = image::load_from_memory(include_bytes!("fixtures/teleport/world.png"))
        .unwrap()
        .to_rgb8();
    let mini = image::load_from_memory(include_bytes!("fixtures/teleport/world-mini.png"))
        .unwrap()
        .to_rgb8();
    let map = image::load_from_memory(include_bytes!("fixtures/teleport/map-zoom-3.png"))
        .unwrap()
        .to_rgb8();
    let map_mini = image::load_from_memory(include_bytes!("fixtures/teleport/map-zoom-3-mini.png"))
        .unwrap()
        .to_rgb8();
    let mut task = RouteTask::prepare(
        "original.json",
        ORIGINAL,
        ExecutionScope::FirstTeleport,
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(RouteFrame {
            captured_at: 1000,
            screen: Some(world.as_raw()),
            minimap: mini.as_raw(),
        }),
    );
    let RouteCommand::Tap { serial, x, y } = task.step(
        1200,
        Conditions::Ready,
        Some(RouteFrame {
            captured_at: 1200,
            screen: Some(world.as_raw()),
            minimap: mini.as_raw(),
        }),
    ) else {
        panic!("{}", task.report().reason)
    };
    assert!((240..400).contains(&x) && (90..230).contains(&y));
    task.input_finished(serial, 1400, true, true);
    let mut command = RouteCommand::Observe;
    for at in [1600, 1800] {
        command = task.step(
            at,
            Conditions::Ready,
            Some(RouteFrame {
                captured_at: at,
                screen: Some(map.as_raw()),
                minimap: map_mini.as_raw(),
            }),
        );
    }
    let RouteCommand::Tap { x, y, .. } = command else {
        panic!("{command:?}: {}", task.report().reason)
    };
    assert!(
        (x as f64 - 1230.0).hypot(y as f64 - 581.0) < 24.0,
        "({x},{y}) must be the observed anchor, not game coordinates"
    );
    assert!(!task.report().teleport_validation_passed);
}

#[test]
fn imported_original_keeps_its_teleport_and_source_identity_before_any_input() {
    let task =
        RouteTask::prepare("original.json", ORIGINAL, ExecutionScope::WholeRoute, 1000).unwrap();
    assert_eq!(
        task.report().source_sha256,
        "1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2"
    );
    assert_eq!(task.report().current_index, 1);
    assert!(!task.report().original_route_passed);
    assert!(task.report().completed_points.is_empty());
    assert_eq!(task.report().command_count, 0);
    for source in [
        br#"{"info":{},"positions":[{"x":267.96,"y":-665.16,"type":"teleport","action":"force_tp"}]}"#.as_slice(),
        br#"{"info":{},"positions":[{"x":366,"y":-502,"type":"teleport"}]}"#,
        br#"{"info":{"map_name":"TheChasm"},"positions":[{"x":267.96,"y":-665.16,"type":"teleport"}]}"#,
    ] {
        assert!(RouteTask::prepare("unsupported.json", source, ExecutionScope::WholeRoute, 1000).is_err());
    }
}
