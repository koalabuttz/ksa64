//! Portable Phase 4 campaign, storage, and statistical-analysis contracts.

pub mod aggregate;
pub mod archive;
pub mod campaign;
pub mod config;
pub mod contracts;
#[cfg(feature = "fixtures")]
pub mod generated_distribution_vectors;
pub mod mission;
pub mod plot;
pub mod reu;
pub mod runner;
pub mod stock;
pub mod stock_ui;
pub mod storage;
pub mod summary;

pub const PHASE4_CONTRACT_ID: u32 = 0x0400_0001;
