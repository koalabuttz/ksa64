//! Portable Phase 4 campaign, storage, and statistical-analysis contracts.

pub mod campaign;
pub mod config;
pub mod contracts;
#[cfg(feature = "fixtures")]
pub mod generated_distribution_vectors;

pub const PHASE4_CONTRACT_ID: u32 = 0x0400_0001;
