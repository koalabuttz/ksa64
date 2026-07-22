#![no_std]

pub mod dynamics;
pub mod environment;
pub mod mission;
pub mod numeric;
pub mod quantities;
pub mod scenario;
pub mod telemetry;
pub mod vehicle;

#[cfg(feature = "c64")]
pub mod c64_status;

#[cfg(feature = "c64")]
pub mod c64_timer;

#[cfg(feature = "fixtures")]
mod self_test;

#[cfg(feature = "fixtures")]
pub use self_test::{run_c64_acceptance_self_tests, run_numeric_self_tests};
