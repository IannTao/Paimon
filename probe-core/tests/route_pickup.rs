use paimon_probe::route_execution::{ExecutionScope, RouteTask};
use paimon_probe::{
    movement::Conditions,
    route_execution::{RouteCommand, RouteFrame},
};

const ORIGINAL: &[u8] = include_bytes!("fixtures/bettergi/route-1.json");

#[test]
fn first_pickup_keeps_the_original_route_identity_and_plans_the_first_target_only() {
    let task =
        RouteTask::prepare("original.json", ORIGINAL, ExecutionScope::FirstPickup, 1000).unwrap();
    let report = serde_json_lenient::to_value(task.report()).unwrap();
    assert_eq!(
        report["source_sha256"],
        "1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2"
    );
    assert_eq!(report["pickup_target_index"], 4);
    assert_eq!(report["last"], 7);
    assert_eq!(report["current_index"], 1);
    assert_eq!(report["pickup_validation_passed"], false);
    assert_eq!(report["original_route_passed"], false);
    let without_target = br#"{"info":{},"positions":[{"x":267.96,"y":-665.16,"type":"teleport"}]}"#;
    assert!(RouteTask::prepare(
        "no-target.json",
        without_target,
        ExecutionScope::FirstPickup,
        1000
    )
    .is_err());
}

#[test]
fn pickup_navigation_approaches_the_original_coordinate_more_closely_than_navigation_alone() {
    // Independent phone localization at the original route's first target (1.875 units away).
    let source = br#"{"info":{},"positions":[{"id":4,"x":279.47,"y":-663.86,"type":"target"}]}"#;
    let mini = image::load_from_memory(include_bytes!("fixtures/pickup/target4-mini.png"))
        .unwrap()
        .to_rgb8();
    let mut task = RouteTask::prepare(
        "single-target.json",
        source,
        ExecutionScope::FirstPickup,
        1000,
    )
    .unwrap();
    for at in [1000, 1200] {
        assert!(matches!(
            task.step(
                at,
                Conditions::Ready,
                Some(RouteFrame {
                    captured_at: at,
                    screen: None,
                    minimap: mini.as_raw()
                })
            ),
            RouteCommand::Observe | RouteCommand::Turn { .. }
        ));
    }
    let report = serde_json_lenient::to_value(task.report()).unwrap();
    assert_eq!(report["phase"], "navigation");
    assert_eq!(report["current_index"], 1);
    assert_eq!(report["pickup_validation_passed"], false);
    assert!(report["completed_points"].as_array().unwrap().is_empty());
    assert_ne!(report["state"], "completed");
}

fn at_target() -> RouteTask {
    // Explicit one-point fixture at the independently localized phone position.
    // The original seven-point file is covered separately; this is not an upstream route.
    let source = br#"{"info":{},"positions":[{"id":4,"x":277.69,"y":-664.46,"type":"target"}]}"#;
    let mut task = RouteTask::prepare(
        "single-target.json",
        source,
        ExecutionScope::FirstPickup,
        1000,
    )
    .unwrap();
    frame(&mut task, 1000, None);
    frame(&mut task, 1200, None);
    task
}

fn frame(task: &mut RouteTask, at: u64, name: Option<&str>) -> RouteCommand {
    let mini = image::load_from_memory(include_bytes!("fixtures/pickup/target4-mini.png"))
        .unwrap()
        .to_rgb8();
    let screen = name.map(|name| {
        image::open(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/fixtures/pickup/{name}.jpg")),
        )
        .unwrap()
        .to_rgb8()
    });
    task.step(
        at,
        Conditions::Ready,
        Some(RouteFrame {
            captured_at: at,
            minimap: mini.as_raw(),
            screen: screen.as_ref().map(|im| im.as_raw().as_slice()),
        }),
    )
}

#[test]
fn a_direct_prompt_and_new_obtained_item_evidence_are_both_required() {
    let mut task = at_target();
    assert_eq!(
        frame(&mut task, 1400, Some("seed-prompt")),
        RouteCommand::Observe
    );
    let RouteCommand::Tap { serial, x, y } = frame(&mut task, 1600, Some("seed-prompt")) else {
        panic!("{}", task.report().reason)
    };
    assert!((1470..1900).contains(&x) && (510..600).contains(&y));
    task.input_finished(serial, 1800, true, true);
    assert!(!task.report().pickup_validation_passed);
    frame(&mut task, 2000, Some("obtained"));
    assert!(!task.report().pickup_validation_passed);
    assert_eq!(frame(&mut task, 2200, Some("obtained")), RouteCommand::End);
    let report = serde_json_lenient::to_value(task.report()).unwrap();
    assert_eq!(report["pickup_validation_passed"], true);
    assert_eq!(report["original_route_passed"], false);
    assert_eq!(report["current_index"], 2);
    assert_eq!(report["pickup_evidence"]["item"], "琉璃百合");
    assert_eq!(report["pickup_evidence"]["quantity"], 1);
    assert_eq!(report["pickup_evidence"]["target_index"], 1);
}

#[test]
fn runtime_nighttime_obtained_pixels_confirm_the_item_despite_different_background_and_video_antialiasing(
) {
    let mut task = at_target();
    frame(&mut task, 1400, Some("seed-prompt"));
    let RouteCommand::Tap { serial, .. } = frame(&mut task, 1600, Some("seed-prompt")) else {
        panic!()
    };
    task.input_finished(serial, 1800, true, true);
    frame(&mut task, 2000, Some("runtime-obtained-1"));
    frame(&mut task, 2200, Some("runtime-obtained-2"));
    assert_eq!(
        frame(&mut task, 2400, Some("runtime-obtained-3")),
        RouteCommand::End
    );
    assert!(task.report().pickup_validation_passed);
}

#[test]
fn unknown_or_absent_prompts_only_allow_six_bounded_camera_searches() {
    for name in ["unknown-label", "no-resource"] {
        let mut task = at_target();
        for n in 0..6 {
            let at = 1400 + n * 400;
            let RouteCommand::Turn { serial, dx } = frame(&mut task, at, Some(name)) else {
                panic!("{}", task.report().reason)
            };
            assert!(dx.abs() <= 240);
            task.input_finished(serial, at + 200, true, true);
        }
        assert_eq!(frame(&mut task, 4000, Some(name)), RouteCommand::End);
        assert!(!task.report().pickup_validation_passed);
        assert!(task.report().reason.contains("搜索"));
    }
}

#[test]
fn disappearance_or_incomplete_obtained_notification_never_proves_an_item() {
    for name in ["no-resource", "no-heading", "no-quantity", "seed-prompt"] {
        let mut task = at_target();
        frame(&mut task, 1400, Some("seed-prompt"));
        let RouteCommand::Tap { serial, .. } = frame(&mut task, 1600, Some("seed-prompt")) else {
            panic!()
        };
        task.input_finished(serial, 1800, true, true);
        for at in [2000, 2200, 2400] {
            frame(&mut task, at, Some(name));
        }
        assert!(!task.report().pickup_validation_passed);
        assert_eq!(task.step(6601, Conditions::Ready, None), RouteCommand::End);
        assert!(task.report().reason.contains("未确认取得"));
    }
}

#[test]
fn an_old_obtained_notification_does_not_count_and_must_clear_before_a_pickup_tap() {
    let mut task = at_target();
    for at in [1400, 1600, 1800] {
        assert_eq!(
            frame(&mut task, at, Some("obtained")),
            RouteCommand::Observe
        );
    }
    assert_eq!(task.report().command_count, 0);
    assert!(!task.report().pickup_validation_passed);
    frame(&mut task, 2000, Some("seed-prompt"));
    assert!(matches!(
        frame(&mut task, 2200, Some("seed-prompt")),
        RouteCommand::Tap { .. }
    ));
}

#[test]
fn every_interruption_cancels_camera_and_pickup_input_and_late_callbacks_cannot_resume() {
    for condition in [
        Conditions::UserStopped,
        Conditions::WrongForeground,
        Conditions::PermissionLost,
        Conditions::Locked,
        Conditions::DisplayChanged,
        Conditions::ScreenLost,
    ] {
        for clicking in [false, true] {
            let mut task = at_target();
            let command = if clicking {
                frame(&mut task, 1400, Some("seed-prompt"));
                frame(&mut task, 1600, Some("seed-prompt"))
            } else {
                frame(&mut task, 1600, Some("unknown-label"))
            };
            let serial = match command {
                RouteCommand::Tap { serial, .. } | RouteCommand::Turn { serial, .. } => serial,
                _ => panic!(),
            };
            assert_eq!(task.step(1700, condition, None), RouteCommand::End);
            task.input_finished(serial, 1800, true, true);
            assert_eq!(frame(&mut task, 2000, Some("obtained")), RouteCommand::End);
            assert!(
                !task.report().pickup_validation_passed && task.report().pickup_evidence.is_none()
            );
        }
    }
}
