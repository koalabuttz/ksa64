use ksa64_sim::phase5_vehicle::{
    Phase5VehicleCommand, Phase5VehicleMachine, Phase6FastVehicle, PHASE5_SUBSTEPS,
};
#[test]
fn four_phase6_ticks_equal_one_frozen_phase5_step() {
    let base = Phase5VehicleMachine::new_ksa5a().unwrap();
    let expected = base;
    let expected = expected;
    let mut legacy = expected;
    let legacy_snapshot = legacy.step(Phase5VehicleCommand::HOLD).unwrap();
    let mut split = Phase6FastVehicle::new(base);
    split.begin(Phase5VehicleCommand::HOLD).unwrap();
    let mut committed = None;
    for _ in 0..PHASE5_SUBSTEPS {
        committed = split.advance(Phase5VehicleCommand::HOLD).unwrap().1
    }
    assert_eq!(committed, Some(legacy_snapshot));
    assert_eq!(split.committed_machine().truth(), legacy.truth())
}
