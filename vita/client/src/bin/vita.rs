//! Placeholder Vita entrypoint.
//!
//! The actual SDL2/Vita platform shell is intentionally deferred until the
//! pinned VitaSDK lane is available. Keeping this executable tiny proves the
//! client crate has no desktop bridge or simulator dependency.

fn main() {
    println!("KSA64 Vita Mission Control platform shell: build the SDL2 adapter with the pinned VitaSDK.");
}
