#![no_std]

//! Phase 3 closed-loop composition layer.
//!
//! This is the only new crate allowed to see both simulator truth and flight
//! software interfaces.

pub use ksa64_core as core_world;
pub use ksa64_flight as flight;
pub use ksa64_interface as interface;

pub mod actuator;
pub mod config;
pub mod mission;
pub mod phase4;
pub mod phase5_closed_loop;
pub mod phase5_sensors;
pub mod phase5_vehicle;
#[cfg(feature = "fixtures")]
mod phase5_vehicle_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_vehicle_self_test::{phase5_vehicle_signature, run_phase5_vehicle_self_tests};
#[cfg(feature = "fixtures")]
mod phase5_avionics_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_avionics_self_test::{phase5_avionics_signature, run_phase5_avionics_self_tests};
pub mod probe;
pub mod replay;
pub mod sensors;
pub mod telemetry;
pub mod world;

pub const PHASE3_SIM_CONTRACT_ID: u32 = 0x0300_0001;
