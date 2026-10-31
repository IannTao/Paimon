use paimon_probe::{
    localization::{LocalizationState, LocalizationTask, MiniMapFrame},
    route_import::import_route,
};

#[test]
fn unsupported_underground_image_does_not_inherit_the_route_start_as_position() {
    let route = import_route(
        "route.json",
        include_bytes!("fixtures/bettergi/route-1.json"),
    );
    let mut task = LocalizationTask::new(&route);
    let image = image::load_from_memory(include_bytes!("fixtures/localization/chasm.png"))
        .unwrap()
        .to_rgb8();
    let result = task.observe(
        1000,
        MiniMapFrame {
            captured_at: 1000,
            rgb: image.as_raw(),
        },
    );
    assert_eq!(result.state, LocalizationState::Unlocated);
    assert!(result.position.is_none());
    assert!(!result.navigation_allowed);
    assert_eq!(result.target.unwrap().x, 267.96);
}

#[test]
fn another_reference_position_and_heading_are_observed_but_sudden_jumps_are_not_trusted() {
    let route = import_route(
        "route.json",
        include_bytes!("fixtures/bettergi/route-1.json"),
    );
    let first = image::load_from_memory(include_bytes!("fixtures/localization/harbor-128.png"))
        .unwrap()
        .to_rgb8();
    let second = image::load_from_memory(include_bytes!("fixtures/localization/harbor-127.png"))
        .unwrap()
        .to_rgb8();
    let mut task = LocalizationTask::new(&route);
    let result = task.observe(
        1000,
        MiniMapFrame {
            captured_at: 1000,
            rgb: second.as_raw(),
        },
    );
    println!("tp127: {result:?}");
    let p = result
        .position
        .expect("The independent second image must match");
    // tp.json id=127, deliberately different from the imported route's first point.
    assert!((p.x - 366.71155).hypot(p.y + 502.4061) < 8.0, "{p:?}");
    let angle = result.heading_degrees.unwrap();
    assert!((angle - 65.0).abs() < 15.0, "{angle}");
    let result = task.observe(
        1200,
        MiniMapFrame {
            captured_at: 1200,
            rgb: first.as_raw(),
        },
    );
    assert!(
        result.position.is_none(),
        "Jump must invalidate the task: {result:?}"
    );
    assert!(!result.navigation_allowed);
}

#[test]
fn actual_harbor_screen_matches_independent_teleport_spawn_coordinates() {
    let route = import_route(
        "route.json",
        include_bytes!("fixtures/bettergi/route-1.json"),
    );
    let mut task = LocalizationTask::new(&route);
    let image = image::load_from_memory(include_bytes!("fixtures/localization/harbor-128.png"))
        .unwrap()
        .to_rgb8();
    let result = task.observe(
        1000,
        MiniMapFrame {
            captured_at: 1000,
            rgb: image.as_raw(),
        },
    );
    assert_eq!(result.state, LocalizationState::Located, "{result:?}");
    let p = result.position.unwrap();
    println!("tp128: {result:?}");
    // Independent tp.json id=128 tranPosition: (X,Y,Z)=(-665.0128,221.2728,267.9441).
    assert!((p.x - 267.9441).hypot(p.y + 665.0128) < 8.0, "{p:?}");
    assert!(result.heading_degrees.is_some());
    assert!(
        (result.heading_degrees.unwrap() - 90.0).abs() < 12.0,
        "{result:?}"
    );
    assert!(!result.navigation_allowed);
}

#[test]
fn expired_replayed_or_wrong_map_screens_cannot_supply_a_current_position() {
    let route = import_route(
        "route.json",
        include_bytes!("fixtures/bettergi/route-1.json"),
    );
    let image = image::load_from_memory(include_bytes!("fixtures/localization/harbor-128.png"))
        .unwrap()
        .to_rgb8();
    let mut task = LocalizationTask::new(&route);
    assert!(task
        .observe(
            1000,
            MiniMapFrame {
                captured_at: 1000,
                rgb: image.as_raw()
            }
        )
        .position
        .is_some());
    assert!(task.snapshot(2600).position.is_none());
    assert!(task
        .observe(
            3000,
            MiniMapFrame {
                captured_at: 1000,
                rgb: image.as_raw()
            }
        )
        .position
        .is_none());
    let unknown = import_route(
        "other.json",
        br#"{"info":{"map_name":"TheChasm"},"positions":[{"x":267,"y":-665}]}"#,
    );
    let mut task = LocalizationTask::new(&unknown);
    assert!(task
        .observe(
            1000,
            MiniMapFrame {
                captured_at: 1000,
                rgb: image.as_raw()
            }
        )
        .position
        .is_none());
    assert!(task
        .observe(
            1000,
            MiniMapFrame {
                captured_at: 1000,
                rgb: &[]
            }
        )
        .position
        .is_none());
}

#[test]
fn blurred_missing_and_arrowless_images_clear_a_previous_fix() {
    let route = import_route(
        "route.json",
        include_bytes!("fixtures/bettergi/route-1.json"),
    );
    let original = image::load_from_memory(include_bytes!("fixtures/localization/harbor-128.png"))
        .unwrap()
        .to_rgb8();
    let blurred = image::imageops::blur(&original, 15.0);
    let mut hidden_arrow = original.clone();
    for y in 112..152 {
        for x in 112..152 {
            hidden_arrow.put_pixel(x, y, image::Rgb([0, 0, 0]));
        }
    }
    for pixels in [&blurred.as_raw()[..], &hidden_arrow.as_raw()[..], &[]] {
        let mut task = LocalizationTask::new(&route);
        assert!(task
            .observe(
                1000,
                MiniMapFrame {
                    captured_at: 1000,
                    rgb: original.as_raw()
                }
            )
            .position
            .is_some());
        let result = task.observe(
            1200,
            MiniMapFrame {
                captured_at: 1200,
                rgb: pixels,
            },
        );
        assert!(result.position.is_none(), "{result:?}");
        assert!(task.snapshot(1200).position.is_none());
        assert!(!result.navigation_allowed);
    }
}

#[test]
fn unsupported_layer_or_matching_configuration_blocks_the_same_real_surface_image() {
    let image = image::load_from_memory(include_bytes!("fixtures/localization/harbor-128.png"))
        .unwrap()
        .to_rgb8();
    for json in [
        r#"{"info":{},"config":{"map_layer":"underground"},"positions":[{"x":268,"y":-665}]}"#,
        r#"{"info":{"map_layer":"underground"},"positions":[{"x":268,"y":-665}]}"#,
        r#"{"info":{},"positions":[{"x":268,"y":-665,"layer":1}]}"#,
        r#"{"info":{},"map_layer":"underground","positions":[{"x":268,"y":-665}]}"#,
    ] {
        let route = import_route("layer.json", json.as_bytes());
        let mut task = LocalizationTask::new(&route);
        let report = task.observe(
            1000,
            MiniMapFrame {
                captured_at: 1000,
                rgb: image.as_raw(),
            },
        );
        assert!(report.position.is_none(), "{json}: {report:?}");
    }
}
