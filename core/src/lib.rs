#![no_std]

pub mod dynamics;
pub mod environment;
pub mod mission;
pub mod numeric;
pub mod quantities;
pub mod scenario;
pub mod vehicle;

#[cfg(feature = "fixtures")]
mod self_test;

#[cfg(feature = "fixtures")]
pub use self_test::run_numeric_self_tests;
