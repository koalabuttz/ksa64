#![no_std]

//! Phase 3 flight software.
//!
//! This crate deliberately depends only on `ksa64-interface`. Simulator truth,
//! vehicle models, and environment models are structurally unavailable here.

pub mod gnc;
pub mod navigation;

pub use ksa64_interface as interface;

pub const FLIGHT_SOFTWARE_CONTRACT_ID: u32 = 0x0300_0001;
