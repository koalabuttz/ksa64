#![no_std]

pub mod numeric;
pub mod quantities;

#[cfg(feature = "fixtures")]
mod self_test;

#[cfg(feature = "fixtures")]
pub use self_test::run_numeric_self_tests;
