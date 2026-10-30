use paimon_probe::movement::{
    Command, Conditions, MovementTask, ScreenFrame, Verdict, HEIGHT, WIDTH,
};

// Known screen sequence: a textured map translates two pixels; the HUD stays fixed.
fn screen(dx: i32, dy: i32) -> Vec<u8> {
    let mut rgb = vec![0; WIDTH * HEIGHT * 3];
    for y in 4..63 {
        for x in 30..94 {
            let a = (x as i32 - dx) as u32;
            let b = (y as i32 - dy) as u32;
            let value = ((a.wrapping_mul(73) ^ b.wrapping_mul(151) ^ (a * b)) % 180 + 40) as u8;
            let i = (y * WIDTH + x) * 3;
            rgb[i..i + 3].fill(value);
        }
    }
    for x in 210..285 {
        let i = (205 * WIDTH + x) * 3;
        rgb[i..i + 3].copy_from_slice(&[130, 210, 25]);
    }
    rgb
}

fn observe(task: &mut MovementTask, now: u64, pixels: &[u8]) -> Command {
    task.advance(
        now,
        Conditions::Ready,
        Some(ScreenFrame {
            captured_at: now,
            rgb: pixels,
        }),
    )
}

#[test]
fn a_task_confirms_movement_from_map_translation_after_releasing_the_joystick() {
    let baseline = screen(0, 0);
    let moved = screen(0, 2);
    let mut task = MovementTask::start(0);
    assert_eq!(observe(&mut task, 10, &baseline), Command::Observe);
    assert_eq!(observe(&mut task, 400, &baseline), Command::Move);
    task.input_finished(1_300, true);
    assert_eq!(observe(&mut task, 1_600, &moved), Command::Observe);
    assert_eq!(
        observe(&mut task, 1_950, &moved),
        Command::End(Verdict::MovementConfirmed)
    );
}

#[test]
fn a_command_returning_success_without_actual_map_displacement_is_not_movement() {
    let baseline = screen(0, 0);
    let mut task = MovementTask::start(0);
    observe(&mut task, 10, &baseline);
    assert_eq!(observe(&mut task, 400, &baseline), Command::Move);
    task.input_finished(1_300, true);
    // The world may animate while map terrain stays still.
    let mut animated_world = baseline.clone();
    animated_world[(100 * WIDTH + 200) * 3..(150 * WIDTH + 200) * 3].fill(250);
    assert_eq!(
        observe(&mut task, 1_600, &animated_world),
        Command::End(Verdict::NoMovementEvidence)
    );
}

#[test]
fn a_non_game_screen_cannot_start_joystick_input() {
    let blank = vec![0; WIDTH * HEIGHT * 3];
    let mut task = MovementTask::start(0);
    assert_eq!(
        observe(&mut task, 10, &blank),
        Command::End(Verdict::InvalidScreen)
    );
}

#[test]
fn stop_or_lost_run_conditions_end_the_task_even_with_input_in_flight() {
    for (condition, verdict) in [
        (Conditions::UserStopped, Verdict::UserStopped),
        (Conditions::PermissionLost, Verdict::PermissionLost),
        (Conditions::ScreenLost, Verdict::ScreenLost),
        (Conditions::WrongForeground, Verdict::WrongForeground),
        (Conditions::DisplayChanged, Verdict::DisplayChanged),
        (Conditions::Locked, Verdict::Locked),
    ] {
        let baseline = screen(0, 0);
        let mut task = MovementTask::start(0);
        observe(&mut task, 10, &baseline);
        assert_eq!(observe(&mut task, 400, &baseline), Command::Move);
        assert_eq!(task.advance(500, condition, None), Command::End(verdict));
        task.input_finished(1_300, true);
        assert_eq!(
            observe(&mut task, 2_000, &screen(0, 2)),
            Command::End(verdict)
        );
    }
}

#[test]
fn stale_screens_and_hung_input_cannot_keep_a_movement_task_alive() {
    let baseline = screen(0, 0);
    let mut stale = MovementTask::start(0);
    assert_eq!(
        stale.advance(
            2_000,
            Conditions::Ready,
            Some(ScreenFrame {
                captured_at: 0,
                rgb: &baseline,
            })
        ),
        Command::End(Verdict::ScreenLost)
    );
    let mut hung = MovementTask::start(0);
    observe(&mut hung, 10, &baseline);
    observe(&mut hung, 400, &baseline);
    assert_eq!(
        hung.advance(8_000, Conditions::Ready, None),
        Command::End(Verdict::TimedOut)
    );
    hung.input_finished(8_100, true);
    assert_eq!(
        observe(&mut hung, 8_200, &screen(0, 2)),
        Command::End(Verdict::TimedOut)
    );
}

#[test]
fn a_map_already_moving_before_input_is_not_a_valid_baseline() {
    let mut task = MovementTask::start(0);
    observe(&mut task, 10, &screen(0, 0));
    assert_eq!(
        observe(&mut task, 400, &screen(2, 0)),
        Command::End(Verdict::UnstableBaseline)
    );
}

#[test]
fn failed_input_cannot_be_overridden_by_a_later_changed_screen() {
    let mut task = MovementTask::start(0);
    observe(&mut task, 10, &screen(0, 0));
    observe(&mut task, 400, &screen(0, 0));
    task.input_finished(1_300, false);
    assert_eq!(
        observe(&mut task, 1_600, &screen(0, 2)),
        Command::End(Verdict::InputFailed)
    );
}

#[test]
fn repeated_or_in_flight_screens_are_not_independent_post_release_evidence() {
    let baseline = screen(0, 0);
    let moved = screen(0, 2);
    let mut task = MovementTask::start(0);
    observe(&mut task, 10, &baseline);
    assert_eq!(
        task.advance(
            400,
            Conditions::Ready,
            Some(ScreenFrame {
                captured_at: 10,
                rgb: &baseline,
            })
        ),
        Command::Observe
    );
    assert_eq!(observe(&mut task, 410, &baseline), Command::Move);
    task.input_finished(1_300, true);
    assert_eq!(
        task.advance(
            1_600,
            Conditions::Ready,
            Some(ScreenFrame {
                captured_at: 1_200,
                rgb: &moved,
            })
        ),
        Command::Observe
    );
    assert_eq!(observe(&mut task, 1_700, &moved), Command::Observe);
    assert_eq!(
        task.advance(
            2_050,
            Conditions::Ready,
            Some(ScreenFrame {
                captured_at: 1_700,
                rgb: &moved,
            })
        ),
        Command::Observe
    );
    assert_eq!(
        observe(&mut task, 2_100, &moved),
        Command::End(Verdict::MovementConfirmed)
    );
}
