#![no_std]

//! Transport-neutral, role-filtered presentation contracts.
//!
//! This crate is deliberately separate from authoritative simulation and
//! canonical evidence formats. It provides bounded presentation DTOs, strict
//! KPS1 framing, and reconnect-friendly retained streams for native, browser,
//! Vita, and future mobile clients.

extern crate alloc;

mod dto;
mod protocol;
mod stream;
mod typed;

pub use dto::*;
pub use protocol::*;
pub use stream::*;
pub use typed::*;
