use ksa64_host::phase6_runner::{run_native_host_mission, RunnerOptions, RunnerPace};

#[test]
fn all_host_mission_control_is_passive_and_complete() {
    let with_control = run_native_host_mission(RunnerOptions {
        mission_control: true,
        pace: RunnerPace::Fast,
    })
    .unwrap();
    let without_control = run_native_host_mission(RunnerOptions {
        mission_control: false,
        pace: RunnerPace::Fast,
    })
    .unwrap();

    assert!(with_control.complete);
    assert_eq!(with_control.fast_epochs, 12_692);
    assert_eq!(with_control.mission_steps, 3_173);
    assert_eq!(
        with_control.terminal_position_q12,
        [21_360_371, 4_030_786, 15_731_027]
    );
    assert_eq!(
        with_control.terminal_velocity_q24,
        [-69_442_203, 96_406_364, 65_655_653]
    );
    assert_eq!(with_control.navigation_checksum, 2_195_755_368);
    assert_eq!(with_control.status_flight_checksum, 833_868_727);
    assert_eq!(with_control.final_flight_checksum, 2_901_449_607);
    assert_eq!(with_control.deadline_misses, 0);
    assert_eq!(with_control.alarms, 0);

    assert_eq!(with_control.fast_epochs, without_control.fast_epochs);
    assert_eq!(with_control.mission_steps, without_control.mission_steps);
    assert_eq!(
        with_control.terminal_position_q12,
        without_control.terminal_position_q12
    );
    assert_eq!(
        with_control.terminal_velocity_q24,
        without_control.terminal_velocity_q24
    );
    assert_eq!(
        with_control.navigation_checksum,
        without_control.navigation_checksum
    );
    assert_eq!(
        with_control.final_flight_checksum,
        without_control.final_flight_checksum
    );
    assert!(without_control.mission_control.is_none());

    let control = with_control.mission_control.unwrap();
    assert_eq!(control.world_cells, 15_865);
    assert_eq!(control.flight_cells, 15_865);
    assert_eq!(control.ground_fixes, 1_587);
    assert_eq!(control.transcript_checksum, 0x233d_4c31);
    assert_eq!(control.ground_checksum, 0x35b6_e19d);
    assert_eq!(control.alarms, 0);
    assert_eq!(
        control.comparison.unwrap().position_delta_q12,
        [15_022, -74_166, 68_209]
    );
    assert_eq!(
        control.comparison.unwrap().velocity_delta_q24,
        [-1_145_424, 845_075, 2_489_450]
    );
}
