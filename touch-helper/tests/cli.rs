use std::process::Command;

#[test]
fn unsafe_or_ambiguous_cli_options_fail_before_any_worker_is_started() {
    for args in [
        vec![
            "forward",
            "--device",
            "/dev/input/event7",
            "--seconds",
            "30",
        ],
        vec![
            "create",
            "--device",
            "/dev/input/event7",
            "--seconds",
            "121",
        ],
        vec!["grab", "--device", "/dev/input/event7", "--seconds", "0"],
        vec![
            "merge",
            "--device",
            "/dev/input/event7/../event1",
            "--seconds",
            "30",
            "--display-verified",
        ],
        vec![
            "grab",
            "--device",
            "/dev/input/event7",
            "--seconds",
            "1",
            "--seconds",
            "3",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_paimon-touch-helper"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8(result.stderr)
            .unwrap()
            .contains("\"event\":\"error\""));
    }
}
