use paimon_probe::route_import::{import_route, ImportStatus};

#[test]
fn original_route_previews_in_array_order_but_cannot_execute() {
    let report = import_route(
        "琉璃百合.json",
        include_bytes!("fixtures/bettergi/route-1.json"),
    );
    assert_eq!(report.status, ImportStatus::Unsupported);
    assert_eq!(report.points.len(), 7);
    assert_eq!(report.points[0].x, 267.96);
    assert_eq!(report.points[0].y, -665.16);
    assert_eq!(report.points[0].point_type, "teleport");
    assert_eq!(report.map_name, "Teyvat");
    assert!(!report.capabilities.is_empty());
    assert!(report
        .capabilities
        .iter()
        .any(|c| c.field == "type:teleport" && !c.executable));
    assert!(report
        .capabilities
        .iter()
        .any(|c| c.field == "move_mode:walk" && c.executable));
    assert!(report
        .capabilities
        .iter()
        .any(|c| c.field == "world_localization" && c.executable));
    assert_eq!(report.original["info"]["authors"][0]["name"], "黎歌");
}

#[test]
fn ground_navigation_is_ready_only_inside_its_verified_scope_without_required_actions() {
    let report = import_route(
        "derived-validation.json",
        include_bytes!("fixtures/navigation/derived-ground-orientation.json"),
    );
    assert_eq!(report.status, ImportStatus::Executable);
    assert!(report.capabilities.iter().all(|c| c.executable));
    let report = import_route(
        "unsupported.json",
        br#"{"info":{},"positions":[{"x":268,"y":-665,"type":"target","action":"pick"}]}"#,
    );
    assert_eq!(report.status, ImportStatus::Unsupported);
}

#[test]
fn localization_capability_does_not_extend_outside_the_verified_surface_region() {
    for route in [
        r#"{"info":{},"positions":[{"x":5000,"y":-665}]}"#,
        r#"{"info":{"map_name":"TheChasm"},"positions":[{"x":268,"y":-665}]}"#,
        r#"{"info":{},"config":{"layer":1},"positions":[{"x":268,"y":-665}]}"#,
    ] {
        let report = import_route("outside.json", route.as_bytes());
        assert_eq!(report.status, ImportStatus::Unsupported);
        assert!(report.capabilities.iter().all(|c| !c.executable));
    }
}

#[test]
fn ambiguous_or_unbounded_files_are_rejected_without_losing_the_failure_reason() {
    for bytes in [
        b"\xff".as_slice(),
        br#"{"info":{},"positions":[{"x":1,"x":2,"y":1}]}"#,
        br#"{"info":{},"positions":[{"x":1,"y":2}]} garbage"#,
    ] {
        assert_eq!(
            import_route("bad.json", bytes).status,
            ImportStatus::Invalid
        );
    }
    assert_eq!(
        import_route("large.json", &vec![b' '; 2 * 1024 * 1024 + 1]).status,
        ImportStatus::Invalid
    );
    let deep = format!("{}0{}", "[".repeat(150), "]".repeat(150));
    assert_eq!(
        import_route("deep.json", deep.as_bytes()).status,
        ImportStatus::Invalid
    );
}

#[test]
fn jsonc_defaults_metadata_and_unrecognized_behavior_are_preserved() {
    let text = br#"{/* comment */"info":{"name":"custom","author":"A","map_name":"Other","version":"9"},
      "config":{"force_party":"x"},"positions":[{"id":9,"x":1.125,"y":-2,"action_params":{"n":3},},
      {"id":1,"x":1.125,"y":-2,"type":"future","move_mode":"hover","action":"new_action","custom":true},],}"#;
    let report = import_route("custom.json", text);
    assert_eq!(report.status, ImportStatus::Unsupported);
    assert_eq!(report.map_name, "Other");
    assert_eq!(report.points[0].point_type, "path");
    assert_eq!(report.points[0].move_mode, "walk");
    assert_eq!(report.points[0].original["id"], 9);
    assert_eq!(report.points[1].original["id"], 1);
    assert_eq!(report.points[0].original["action_params"]["n"], 3);
    assert_eq!(report.original["info"]["author"], "A");
    assert!(report.upstream_commit.is_none());
    for field in ["info.map_name", "config.force_party", "positions[1].custom"] {
        assert!(
            report.diagnostics.iter().any(|d| d.field == field),
            "{field}"
        );
    }
    assert!(report.capabilities.iter().any(|c| !c.recognized));
}

#[test]
fn invalid_structure_or_coordinates_never_become_ready() {
    for text in [
        r#"{"info":{},"positions":[]}"#,
        r#"{"positions":[{"x":1,"y":2}]}"#,
        r#"{"info":{},"positions":[{"y":1}]}"#,
        r#"{"info":{},"positions":[{"x":"NaN","y":1}]}"#,
        r#"{"info":{},"positions":[{"x":1e999,"y":1}]}"#,
        r#"{"info":{},"positions":[null]}"#,
        r#"{"info":{},"positions":[{"x":1,"y":2,"move_mode":8}]}"#,
    ] {
        let report = import_route("bad.json", text.as_bytes());
        assert_eq!(report.status, ImportStatus::Invalid, "{text}");
        assert!(!report.diagnostics.is_empty());
    }
}

#[test]
fn flight_route_keeps_duplicate_positions_and_reports_each_required_behavior() {
    let report = import_route(
        "flight.json",
        include_bytes!("fixtures/bettergi/route-2.json"),
    );
    assert_eq!(report.points.len(), 20);
    assert_eq!(report.points[1].x, report.points[2].x);
    assert_eq!(report.points[2].action, "stop_flying");
    for field in [
        "positions[0].type",
        "positions[1].move_mode",
        "positions[2].action",
    ] {
        assert!(
            report.diagnostics.iter().any(|d| d.field == field),
            "{field}"
        );
    }
    assert_eq!(
        report.sha256,
        "a2e2bb2ebc659f6013126320d52b53a1c68b4126b24c9e74e59270d36931e6fd"
    );
    assert_eq!(
        report.upstream_commit.as_deref(),
        Some("84c7e15e6096eb6c313b1b525ca7b38298971425")
    );
}
