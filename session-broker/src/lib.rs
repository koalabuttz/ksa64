#![cfg_attr(not(feature = "std"), no_std)]

//! Bounded authority brokerage and paired presentation transport.
//!
//! The Noise XX/IK and encrypted KPS1 fragment layer is `no_std + alloc` so constrained
//! clients use the same cryptographic transport. Browser, socket, worker, and portable-session
//! adapters are available behind the `std` feature and own no simulation state.

extern crate alloc;
#[cfg(test)]
extern crate std;

#[cfg(feature = "std")]
mod browser;
#[cfg(feature = "std")]
mod config;
#[cfg(feature = "std")]
mod lan;
#[cfg(feature = "network")]
mod network;
mod noise;
#[cfg(feature = "std")]
mod paired_state;
#[cfg(feature = "portable-session")]
mod portable_adapter;
#[cfg(feature = "std")]
mod worker;

#[cfg(feature = "std")]
pub use browser::*;
#[cfg(feature = "std")]
pub use config::*;
#[cfg(feature = "std")]
pub use lan::*;
#[cfg(feature = "network")]
pub use network::*;
pub use noise::*;
#[cfg(feature = "std")]
pub use paired_state::*;
#[cfg(feature = "portable-session")]
pub use portable_adapter::*;
#[cfg(feature = "std")]
pub use worker::*;

pub const MAX_PAIRED_PEERS: usize = 16;

/// Compile-time proof that the optional broker adapter links the portable session authority.
#[cfg(feature = "portable-session")]
pub const PORTABLE_SESSION_CRATE_ID: &str = "ksa64-session";
