//! Frozen built-in assets shared by product-facing mission and campaign adapters.

use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase8_format::{KMC8_LENGTH, KMP8_LENGTH, KVP8_LENGTH, KWP8_LENGTH};

pub(crate) const PHASE7_VEHICLE: &[u8; KVP7_LENGTH] =
    include_bytes!("../../phase7/examples/firestorm54.kvp7");
pub(crate) const PHASE7_MOTOR: &[u8; KMP7_LENGTH] =
    include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
pub(crate) const PHASE7_MISSION: &[u8; KMC7_LENGTH] =
    include_bytes!("../../phase7/examples/firestorm-i211.kmc7");
pub(crate) const PHASE8_VEHICLE: &[u8; KVP8_LENGTH] =
    include_bytes!("../../phase8/examples/firestorm54.kvp8");
pub(crate) const PHASE8_MOTOR: &[u8; KMP8_LENGTH] =
    include_bytes!("../../phase8/examples/aerotech-i211w.kmp8");
pub(crate) const PHASE8_MISSION: &[u8; KMC8_LENGTH] =
    include_bytes!("../../phase8/examples/firestorm-i211.kmc8");
pub(crate) const PHASE8_WIND: &[u8; KWP8_LENGTH] =
    include_bytes!("../../phase8/examples/firestorm-calm.kwp8");

pub(crate) const GNSS_LOSS_SOURCE: &str = include_str!("../../phase11/examples/gnss-loss.json");
pub(crate) const SAFEHOLD_SOURCE: &str =
    include_str!("../../phase11/examples/safehold-recovery.json");
