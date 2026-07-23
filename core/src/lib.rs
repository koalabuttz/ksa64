#![no_std]

pub mod aerodynamics;
pub mod dynamics;
pub mod environment;
pub mod guidance;
pub mod mission;
pub mod numeric;
pub mod phase2_mission;
pub mod phase2_numeric;
pub mod phase2_quantities;
pub mod phase2_scenario;
pub mod planar;
pub mod planar_dynamics;
pub mod planar_environment;
pub mod quantities;
pub mod scenario;
pub mod telemetry;
pub mod vehicle;

#[cfg(feature = "c64")]
pub mod c64_status;

#[cfg(feature = "c64")]
pub mod c64_timer;

#[cfg(feature = "fixtures")]
mod phase2_atmosphere_self_test;

#[cfg(feature = "fixtures")]
mod phase2_self_test;

#[cfg(feature = "fixtures")]
mod phase2_scenario_self_test;

#[cfg(feature = "fixtures")]
mod phase2_mission_self_test;

#[cfg(feature = "fixtures")]
mod self_test;

#[cfg(feature = "fixtures")]
pub use phase2_atmosphere_self_test::run_phase2_atmosphere_self_tests;
#[cfg(feature = "fixtures")]
pub use phase2_mission_self_test::{
    run_phase2_failure_mission_self_tests, run_phase2_nominal_mission_self_tests,
};
#[cfg(feature = "fixtures")]
pub use phase2_scenario_self_test::run_phase2_scenario_self_tests;
#[cfg(feature = "fixtures")]
pub use phase2_self_test::run_phase2_contract_self_tests;
#[cfg(feature = "fixtures")]
pub use self_test::{run_c64_acceptance_self_tests, run_numeric_self_tests};
