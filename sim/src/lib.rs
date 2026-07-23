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
pub mod sensors;
pub mod world;

pub const PHASE3_SIM_CONTRACT_ID: u32 = 0x0300_0001;
