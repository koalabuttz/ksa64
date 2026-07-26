#![no_std]

//! Phase 3 closed-loop composition layer.
//!
//! This is the only new crate allowed to see both simulator truth and flight
//! software interfaces.

pub use ksa64_core as core_world;
pub use ksa64_flight as flight;
pub use ksa64_interface as interface;
pub mod evaluation;

pub mod actuator;
pub mod config;
pub mod mission;
pub mod phase4;
pub mod phase5_archive;
pub mod phase5_campaign;
#[cfg(feature = "fixtures")]
mod phase5_campaign_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_campaign_self_test::{
    phase5_campaign_probe_signature, run_phase5_campaign_self_tests,
};
pub mod phase5_closed_loop;
pub mod phase5_history;
#[cfg(feature = "fixtures")]
mod phase5_history_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_history_self_test::{phase5_history_probe_signature, run_phase5_history_self_tests};
pub mod phase5_mission;
pub mod phase5_replay;
pub mod phase5_sensors;
pub mod phase5_telemetry;
#[cfg(feature = "fixtures")]
mod phase5_telemetry_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_telemetry_self_test::{
    phase5_telemetry_codec_signature, run_phase5_telemetry_self_tests,
};
pub mod phase10;
pub mod phase10_avionics;
pub mod phase10_control;
pub mod phase10_evaluation;
pub mod phase5_vehicle;
#[cfg(feature = "fixtures")]
mod phase5_vehicle_self_test;
#[cfg(feature = "c64")]
pub mod phase6_c64;
pub mod phase6_link;
pub mod phase6_mission_control;
pub mod phase6_realtime;
pub mod phase7_campaign;
pub mod phase8_5;
pub mod phase8_campaign;
pub mod phase8_storage;
pub mod phase9_5;
pub mod phase9_5_bootstrap;
#[cfg(feature = "fixtures")]
mod phase9_5_contract_self_test;
pub mod phase9_5_mission;
#[cfg(feature = "fixtures")]
pub use phase5_vehicle_self_test::{phase5_vehicle_signature, run_phase5_vehicle_self_tests};
#[cfg(feature = "fixtures")]
pub use phase9_5_contract_self_test::{
    phase95_contract_signature, run_phase95_contract_self_tests,
};
#[cfg(feature = "fixtures")]
mod phase5_avionics_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_avionics_self_test::{phase5_avionics_signature, run_phase5_avionics_self_tests};
#[cfg(feature = "fixtures")]
mod phase5_guidance_self_test;
#[cfg(feature = "fixtures")]
pub use phase5_guidance_self_test::{phase5_guidance_signature, run_phase5_guidance_self_tests};
pub mod probe;
pub mod replay;
pub mod sensors;
pub mod telemetry;
pub mod world;

pub const PHASE3_SIM_CONTRACT_ID: u32 = 0x0300_0001;
