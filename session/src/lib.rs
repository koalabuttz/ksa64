//! Portable deterministic mission-session authority.
//!
//! This crate owns exact release advancement, procedures, predictions, action
//! evidence, role policy, and in-memory KSB11 finalization. Platform persistence
//! remains in `ksa64-host`, so browser and constrained clients use the same authority.

pub mod global_display;
pub mod global_fixtures;
pub mod phase10_mission;
pub mod phase10_nominal_compat;
pub mod phase11_authoring;
pub mod phase11_debrief;
pub mod phase11_live;
pub mod phase11_operations;
pub mod phase11_prediction;
pub mod phase11_scenarios;
pub mod phase11_session;
pub mod phase12b;
pub mod phase12b_live;
pub mod presentation_adapter;
pub mod presentation_replay;

#[cfg(test)]
mod application_fixtures;
