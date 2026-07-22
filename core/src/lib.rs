#![no_std]

pub mod numeric;
pub mod quantities;
pub mod scenario;

#[cfg(feature = "fixtures")]
mod self_test;

#[cfg(feature = "fixtures")]
pub use self_test::run_numeric_self_tests;
