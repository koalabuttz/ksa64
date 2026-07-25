#![no_std]

pub mod aerodynamics;
pub mod dynamics;
pub mod environment;
pub mod evaluation;
pub mod flexible;
pub mod guidance;
pub mod mission;
pub mod numeric;
pub mod phase2_mission;
pub mod phase2_numeric;
pub mod phase2_quantities;
pub mod phase2_scenario;
pub mod phase2_telemetry;
pub mod phase5_contract;
pub mod phase7_environment;
pub mod phase7_format;
pub mod phase7_mission;
pub mod phase7_numeric;
pub mod phase7_pack;
pub mod phase7_result;
pub mod phase7_telemetry;
pub mod phase8_format;
pub mod phase8_numeric;
pub mod phase8_pack;
pub mod planar;
pub mod planar_dynamics;
pub mod planar_environment;
pub mod quantities;
pub mod rigid_body;
pub mod scenario;
pub mod spatial_numeric;
pub mod spatial_world;
pub mod telemetry;
pub mod vehicle;

#[cfg(feature = "c64")]
pub mod c64_status;

#[cfg(feature = "c64")]
pub mod c64_timer;
#[cfg(feature = "c64")]
#[path = "phase2_c64_replay_tape.rs"]
pub mod phase2_c64_replay;

#[cfg(feature = "fixtures")]
mod phase2_atmosphere_self_test;

#[cfg(feature = "fixtures")]
mod phase2_self_test;

#[cfg(feature = "fixtures")]
mod phase2_scenario_self_test;

#[cfg(feature = "fixtures")]
mod phase2_telemetry_self_test;

#[cfg(feature = "fixtures")]
mod phase2_mission_self_test;

#[cfg(feature = "fixtures")]
mod phase5_flexible_self_test;
#[cfg(feature = "fixtures")]
mod phase8_contract_self_test;

#[cfg(feature = "fixtures")]
mod phase5_rigid_self_test;

#[cfg(feature = "fixtures")]
mod phase5_world_self_test;

#[cfg(feature = "fixtures")]
mod phase5_spatial_self_test;

#[cfg(feature = "fixtures")]
mod self_test;

#[cfg(feature = "fixtures")]
pub use phase2_atmosphere_self_test::run_phase2_atmosphere_self_tests;
#[cfg(feature = "fixtures")]
pub use phase2_mission_self_test::{
    run_phase2_failure_mission_self_tests, run_phase2_mission_smoke_self_tests,
    run_phase2_nominal_mission_self_tests,
};
#[cfg(feature = "fixtures")]
pub use phase2_scenario_self_test::run_phase2_scenario_self_tests;
#[cfg(feature = "fixtures")]
pub use phase2_self_test::run_phase2_contract_self_tests;
#[cfg(feature = "fixtures")]
pub use phase2_telemetry_self_test::run_phase2_telemetry_self_tests;
#[cfg(feature = "fixtures")]
pub use phase5_flexible_self_test::run_phase5_flexible_self_tests;
#[cfg(feature = "fixtures")]
pub use phase5_rigid_self_test::{
    run_phase5_rigid_asymmetric_self_test, run_phase5_rigid_self_tests,
    run_phase5_rigid_spherical_self_test,
};
#[cfg(feature = "fixtures")]
pub use phase5_spatial_self_test::run_phase5_spatial_self_tests;
#[cfg(feature = "fixtures")]
pub use phase5_world_self_test::{phase5_world_signature, run_phase5_world_self_tests};
#[cfg(feature = "fixtures")]
pub use phase8_contract_self_test::{phase8_contract_signature, run_phase8_contract_self_tests};
#[cfg(feature = "fixtures")]
pub use self_test::{run_c64_acceptance_self_tests, run_numeric_self_tests};
