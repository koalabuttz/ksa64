#![no_std]

//! Phase 3 flight software.
//!
//! This crate deliberately depends only on `ksa64-interface`. Simulator truth,
//! vehicle models, and environment models are structurally unavailable here.

pub mod gnc;
pub mod navigation;
pub mod phase10;
pub mod phase11;
pub mod phase11_safehold;
pub mod phase5_gnc;
pub mod phase5_guidance;
pub mod phase5_navigation;
pub mod phase6_realtime;
pub mod phase8_5;
pub mod phase9_5;
pub mod phase9_5_allocator;

pub use ksa64_interface as interface;

pub const FLIGHT_SOFTWARE_CONTRACT_ID: u32 = 0x0300_0001;
