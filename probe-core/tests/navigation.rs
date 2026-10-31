use paimon_probe::navigation::{NavigationScope, NavigationTask};
use paimon_probe::{localization::MiniMapFrame, movement::Conditions};

const ORIGINAL: &[u8] = include_bytes!("fixtures/bettergi/route-1.json");

#[test]
fn navigation_rejects_unbounded_progress_metadata_without_changing_the_imported_file() {
    use paimon_probe::route_import::{import_route, ImportStatus};
    use serde_json_lenient::json;
    for points in [
        vec![json!({"x":267,"y":-664}); 257],
        vec![json!({"id":"a".repeat(1025),"x":267,"y":-664})],
    ] {
        let bytes = serde_json_lenient::to_vec(&json!({"info":{},"positions":points})).unwrap();
        let imported = import_route("large-progress.json", &bytes);
        assert_eq!(imported.status, ImportStatus::Unsupported);
        assert_eq!(imported.points.len(), points.len());
        assert!(NavigationTask::prepare(
            "large-progress.json",
            &bytes,
            NavigationScope::WholeRoute,
            1000
        )
        .is_err());
    }
}

#[test]
fn orientation_followed_by_movement_starts_the_next_leg_at_the_observed_position() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let source = br#"{"info":{},"positions":[{"x":207,"y":-711,"type":"orientation"},{"x":267,"y":-644,"type":"target"}]}"#;
    let before = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    let after = image::load_from_memory(include_bytes!("fixtures/navigation/camera-after.png"))
        .unwrap()
        .to_rgb8();
    let mut task = NavigationTask::prepare(
        "turn-then-walk.json",
        source,
        NavigationScope::WholeRoute,
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: before.as_raw(),
        }),
    );
    let NavigationCommand::Turn { serial, .. } = task.step(
        1200,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1200,
            rgb: before.as_raw(),
        }),
    ) else {
        panic!()
    };
    task.input_finished(serial, 1400, true, true);
    for at in [1600, 1800, 2000] {
        task.step(
            at,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: at,
                rgb: after.as_raw(),
            }),
        );
    }
    assert_eq!(task.report().current_index, 2);
    assert_ne!(
        task.report().state,
        NavigationState::Failed,
        "{}",
        task.report().reason
    );
    assert_eq!(task.report().completed_points.len(), 1);
}

#[test]
fn late_or_backwards_receipts_cannot_bypass_a_deadline_before_the_next_observation() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let image = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    for (tick, receipt) in [(1300, 2201), (1500, 1400), (1300, 122000)] {
        let mut task = NavigationTask::prepare(
            "original.json",
            ORIGINAL,
            NavigationScope::Segment { first: 3, last: 4 },
            1000,
        )
        .unwrap();
        task.step(
            1000,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: 1000,
                rgb: image.as_raw(),
            }),
        );
        let NavigationCommand::Turn { serial, .. } = task.step(
            1200,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: 1200,
                rgb: image.as_raw(),
            }),
        ) else {
            panic!()
        };
        task.step(tick, Conditions::Ready, None);
        task.input_finished(serial, receipt, true, true);
        assert_eq!(task.report().state, NavigationState::Failed);
        assert_eq!(
            task.step(
                receipt.max(tick) + 200,
                Conditions::Ready,
                Some(MiniMapFrame {
                    captured_at: receipt.max(tick) + 200,
                    rgb: image.as_raw()
                })
            ),
            NavigationCommand::End
        );
    }
}

#[test]
fn complete_phone_run_replays_original_bytes_and_real_pixels_through_the_task_entry() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    use serde_json_lenient::Value;
    let recorded: Value = serde_json_lenient::from_slice(include_bytes!(
        "../../docs/validation/reports/issue-18/orientation.json"
    ))
    .unwrap();
    let history = recorded["history"].as_array().unwrap();
    let mut task = NavigationTask::prepare(
        "derived-ground-orientation.json",
        include_bytes!("fixtures/navigation/derived-ground-orientation.json"),
        NavigationScope::WholeRoute,
        history[0]["observedAt"].as_u64().unwrap(),
    )
    .unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/navigation/orientation-run");
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    files.sort();
    let mut images = files.iter();
    let mut pending = None;
    for entry in &history[1..] {
        let now = entry["observedAt"].as_u64().unwrap();
        if let Some(serial) = pending.take() {
            assert!(entry["task"]["input_released"].as_bool().unwrap());
            task.input_finished(serial, now, true, true);
            continue;
        }
        let file = images
            .next()
            .expect("Every observation has an actual captured image");
        let at = file.file_stem().unwrap().to_str().unwrap().parse().unwrap();
        let rgb = image::open(file).unwrap().to_rgb8();
        let command = task.step(
            now,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: at,
                rgb: rgb.as_raw(),
            }),
        );
        assert_eq!(
            serde_json_lenient::to_value(command).unwrap(),
            entry["command"],
            "image {}",
            file.display()
        );
        if task.report().current_index == 4 {
            assert!(!matches!(command, NavigationCommand::Move { .. }));
        }
        if let NavigationCommand::Turn { serial, .. } | NavigationCommand::Move { serial, .. } =
            command
        {
            pending = Some(serial);
        }
    }
    assert!(images.next().is_none());
    assert_eq!(task.report().state, NavigationState::Completed);
    assert_eq!(task.report().completed_points.len(), 4);
    assert_eq!(task.report().move_count, 17);
    assert!(task.report().completed_points[2].distance <= 2.5);
    assert!(
        task.report().completed_points[3]
            .camera_error
            .unwrap()
            .abs()
            <= 12.0
    );
}

#[test]
fn stationary_camera_turn_uses_all_map_inliers_instead_of_jumping_between_two_feature_fits() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let before = image::load_from_memory(include_bytes!("fixtures/navigation/jitter-before.png"))
        .unwrap()
        .to_rgb8();
    let after = image::load_from_memory(include_bytes!("fixtures/navigation/jitter-after.png"))
        .unwrap()
        .to_rgb8();
    let mut task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 3, last: 4 },
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: before.as_raw(),
        }),
    );
    let NavigationCommand::Turn { serial, .. } = task.step(
        1200,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1200,
            rgb: before.as_raw(),
        }),
    ) else {
        panic!()
    };
    task.input_finished(serial, 1400, true, true);
    task.step(
        1600,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1600,
            rgb: after.as_raw(),
        }),
    );
    assert_ne!(
        task.report().state,
        NavigationState::Failed,
        "{}",
        task.report().reason
    );
    assert!(task.report().position.is_some());
}

#[test]
fn the_player_arrow_remains_observable_after_a_real_walk_changes_character_facing() {
    let frame =
        image::load_from_memory(include_bytes!("fixtures/navigation/ground-after-move.png"))
            .unwrap()
            .to_rgb8();
    let mut task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 3, last: 4 },
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: frame.as_raw(),
        }),
    );
    assert!(task.report().position.is_some(), "{}", task.report().reason);
    assert!(
        task.report().character_heading_degrees.is_none(),
        "Ambiguous marker facing must not become navigation direction"
    );
    assert!(task.report().camera_heading_degrees.is_none());
}

#[test]
fn camera_changes_on_the_ground_do_not_lose_a_screen_verified_local_map_track() {
    let before =
        image::load_from_memory(include_bytes!("fixtures/navigation/ground-before-turn.png"))
            .unwrap()
            .to_rgb8();
    let after =
        image::load_from_memory(include_bytes!("fixtures/navigation/ground-after-turn.png"))
            .unwrap()
            .to_rgb8();
    let mut task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 3, last: 4 },
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: before.as_raw(),
        }),
    );
    let initial = task.report().position.expect("Actual pre-turn ground fix");
    task.step(
        1200,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1200,
            rgb: after.as_raw(),
        }),
    );
    let final_position = task
        .report()
        .position
        .expect("Fresh image must independently match nearby atlas features");
    assert!((initial.x - final_position.x).hypot(initial.y - final_position.y) < 1.5);
}

#[test]
fn required_teleport_blocks_the_original_route_but_an_explicit_ground_segment_keeps_its_identity() {
    let rejected =
        NavigationTask::prepare("original.json", ORIGINAL, NavigationScope::WholeRoute, 1000)
            .err()
            .expect("Teleport is not implemented");
    assert!(rejected.contains("teleport"), "{rejected}");
    let task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 2, last: 4 },
        1000,
    )
    .expect("Explicit ground-only validation is allowed");
    let report = task.report();
    assert!(!report.original_route_passed);
    assert_eq!(
        report.source_sha256,
        "1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2"
    );
    assert_eq!(report.first, 2);
    assert_eq!(report.last, 4);
    assert_eq!(report.current_index, 2);
    assert!(report.completed_points.is_empty());
}

#[test]
fn camera_direction_requires_two_observed_frames_around_a_released_turn() {
    use paimon_probe::navigation::NavigationCommand;
    let mut task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 3, last: 4 },
        1000,
    )
    .unwrap();
    let before = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    let after = image::load_from_memory(include_bytes!("fixtures/navigation/camera-after.png"))
        .unwrap()
        .to_rgb8();
    assert_eq!(
        task.step(
            1000,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: 1000,
                rgb: before.as_raw()
            })
        ),
        NavigationCommand::Observe
    );
    assert!(
        task.report().camera_heading_degrees.is_none(),
        "Character heading is not camera feedback"
    );
    let command = task.step(
        1200,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1200,
            rgb: before.as_raw(),
        }),
    );
    let NavigationCommand::Turn { serial, dx } = command else {
        panic!("{command:?}")
    };
    assert_eq!(dx, 160);
    task.input_finished(serial, 1400, true, true);
    task.step(
        2800,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 2600,
            rgb: after.as_raw(),
        }),
    );
    println!("camera pose: {:?}", task.report());
    let camera = task
        .report()
        .camera_heading_degrees
        .expect("A real released turn supplies the fan difference");
    assert!((camera - 135.0).abs() < 15.0, "{camera}");
    assert_eq!(task.report().current_index, 3);
    assert!(task.report().completed_points.is_empty());
}

#[test]
fn duplicate_path_and_target_need_separate_evidence_and_orientation_only_turns_the_view() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let bytes = br#"{"info":{"name":"ground semantics"},"positions":[
        {"id":9,"x":267.9441,"y":-665.0128,"type":"path"},
        {"id":1,"x":267.9441,"y":-665.0128,"type":"target"},
        {"id":8,"x":257.9441,"y":-665.0128,"type":"orientation"}]}"#;
    let mut task =
        NavigationTask::prepare("semantics.json", bytes, NavigationScope::WholeRoute, 1000)
            .unwrap();
    let before = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    let after = image::load_from_memory(include_bytes!("fixtures/navigation/camera-after.png"))
        .unwrap()
        .to_rgb8();
    for at in [1000, 1200, 1400, 1600] {
        assert_eq!(
            task.step(
                at,
                Conditions::Ready,
                Some(MiniMapFrame {
                    captured_at: at,
                    rgb: before.as_raw()
                })
            ),
            NavigationCommand::Observe
        );
    }
    assert_eq!(
        task.report()
            .completed_points
            .iter()
            .map(|p| p.original_id.clone())
            .collect::<Vec<_>>(),
        vec![serde_json_lenient::json!(9), serde_json_lenient::json!(1)]
    );
    let command = task.step(
        1800,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1800,
            rgb: before.as_raw(),
        }),
    );
    let NavigationCommand::Turn { serial, .. } = command else {
        panic!("{command:?}")
    };
    task.input_finished(serial, 2000, true, true);
    let command = task.step(
        2200,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 2200,
            rgb: after.as_raw(),
        }),
    );
    let NavigationCommand::Turn { serial, dx } = command else {
        panic!("{command:?}")
    };
    assert!(dx < 0);
    task.input_finished(serial, 2400, true, true);
    task.step(
        2600,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 2600,
            rgb: before.as_raw(),
        }),
    );
    task.step(
        2800,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 2800,
            rgb: before.as_raw(),
        }),
    );
    assert_eq!(
        task.report().state,
        NavigationState::Completed,
        "{:?}",
        task.report()
    );
    assert_eq!(task.report().completed_points.len(), 3);
    assert!(task.report().original_route_passed);
    assert_eq!(task.report().move_count, 0);
}

#[test]
fn stopping_or_losing_fresh_frames_while_input_is_pending_cannot_be_undone_by_a_late_receipt() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let image = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    for (condition, time) in [
        (Conditions::UserStopped, 1300),
        (Conditions::Ready, 4000),
        (Conditions::WrongForeground, 1300),
        (Conditions::PermissionLost, 1300),
        (Conditions::ScreenLost, 1300),
        (Conditions::DisplayChanged, 1300),
        (Conditions::Locked, 1300),
    ] {
        let mut task = NavigationTask::prepare(
            "original.json",
            ORIGINAL,
            NavigationScope::Segment { first: 3, last: 4 },
            1000,
        )
        .unwrap();
        task.step(
            1000,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: 1000,
                rgb: image.as_raw(),
            }),
        );
        let NavigationCommand::Turn { serial, .. } = task.step(
            1200,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: 1200,
                rgb: image.as_raw(),
            }),
        ) else {
            panic!()
        };
        assert_eq!(task.step(time, condition, None), NavigationCommand::End);
        assert!(matches!(
            task.report().state,
            NavigationState::Stopped | NavigationState::Failed
        ));
        assert!(task.report().position.is_none());
        task.input_finished(serial, time + 100, true, true);
        assert_eq!(
            task.step(
                time + 200,
                Conditions::Ready,
                Some(MiniMapFrame {
                    captured_at: time + 200,
                    rgb: image.as_raw()
                })
            ),
            NavigationCommand::End
        );
        assert!(task.report().completed_points.is_empty());
    }
}

#[test]
fn unsupported_modes_actions_and_route_configuration_cannot_be_silently_skipped() {
    for bytes in [
        br#"{"info":{},"positions":[{"x":268,"y":-665,"type":"path","move_mode":"fly"}]}"#.as_slice(),
        br#"{"info":{},"positions":[{"x":268,"y":-665,"type":"target","action":"pick"}]}"#,
        br#"{"info":{},"config":{"unknown":true},"positions":[{"x":268,"y":-665}]}"#,
        br#"{"info":{},"positions":[{"x":268,"y":-665,"type":"path","action_params":{"duration":1}}]}"#,
    ] {
        assert!(NavigationTask::prepare("unsupported.json", bytes, NavigationScope::WholeRoute, 1000).is_err());
    }
    for scope in [
        NavigationScope::Segment { first: 0, last: 3 },
        NavigationScope::Segment { first: 2, last: 8 },
        NavigationScope::Segment { first: 4, last: 2 },
    ] {
        assert!(NavigationTask::prepare("original.json", ORIGINAL, scope, 1000).is_err());
    }
}

#[test]
fn a_real_image_off_the_current_leg_and_unknown_images_stop_without_progress() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let initial = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    let other = image::load_from_memory(include_bytes!("fixtures/localization/harbor-127.png"))
        .unwrap()
        .to_rgb8();
    let mut task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 3, last: 4 },
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: initial.as_raw(),
        }),
    );
    let command = task.step(
        21000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 21000,
            rgb: other.as_raw(),
        }),
    );
    assert_eq!(command, NavigationCommand::End);
    assert_eq!(task.report().state, NavigationState::Failed);
    assert!(
        task.report().reason.contains("偏离"),
        "{}",
        task.report().reason
    );
    assert!(task.report().completed_points.is_empty());

    let mut task = NavigationTask::prepare(
        "original.json",
        ORIGINAL,
        NavigationScope::Segment { first: 3, last: 4 },
        1000,
    )
    .unwrap();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: initial.as_raw(),
        }),
    );
    let unknown = vec![0; 264 * 264 * 3];
    for at in [1200, 1400, 1600] {
        task.step(
            at,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: at,
                rgb: &unknown,
            }),
        );
    }
    assert_eq!(task.report().state, NavigationState::Failed);
    assert_eq!(task.report().command_count, 0);
}

#[test]
fn successful_motion_receipts_without_screen_progress_stop_instead_of_marking_the_target_reached() {
    use paimon_probe::navigation::{NavigationCommand, NavigationState};
    let route = br#"{"info":{},"positions":[{"x":257.0,"y":-673.0,"type":"target"}]}"#;
    let mut task =
        NavigationTask::prepare("blocked.json", route, NavigationScope::WholeRoute, 1000).unwrap();
    let before = image::load_from_memory(include_bytes!("fixtures/navigation/camera-before.png"))
        .unwrap()
        .to_rgb8();
    let after = image::load_from_memory(include_bytes!("fixtures/navigation/camera-after.png"))
        .unwrap()
        .to_rgb8();
    task.step(
        1000,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1000,
            rgb: before.as_raw(),
        }),
    );
    let NavigationCommand::Turn { serial, .. } = task.step(
        1200,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1200,
            rgb: before.as_raw(),
        }),
    ) else {
        panic!()
    };
    task.input_finished(serial, 1400, true, true);
    let mut command = task.step(
        1600,
        Conditions::Ready,
        Some(MiniMapFrame {
            captured_at: 1600,
            rgb: after.as_raw(),
        }),
    );
    for attempt in 0..4 {
        let NavigationCommand::Move {
            serial,
            duration_ms,
        } = command
        else {
            panic!("{command:?}")
        };
        assert!((120..=400).contains(&duration_ms));
        let at = 2100 + attempt * 700;
        task.input_finished(serial, at, true, true);
        command = task.step(
            at + 150,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: at + 150,
                rgb: after.as_raw(),
            }),
        );
    }
    assert_eq!(command, NavigationCommand::End);
    assert_eq!(task.report().state, NavigationState::Failed);
    assert!(task.report().reason.contains("无进展"));
    assert!(task.report().completed_points.is_empty());
}
