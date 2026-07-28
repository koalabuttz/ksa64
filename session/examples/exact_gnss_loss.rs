//! Host-only exact Phase 12B/12B.5 GNSS-loss evidence writer.
//!
//! This acceptance utility keeps filesystem ownership outside the portable
//! session library while exercising that library's real in-memory finalizer.

use ksa64_interface::phase11::OperationalRole;
use ksa64_session::phase11_session::{sha256, verify_complete_session};
use ksa64_session::phase12b_live::FullMissionSession;
use std::{env, fs, path::PathBuf};

const EXPECTED_LENGTH: usize = 2_911_464;
const EXPECTED_SHA256: [u8; 32] = [
    0x75, 0x54, 0x11, 0x1f, 0x28, 0xd8, 0xf3, 0x62, 0x8a, 0xe3, 0xca, 0x9d, 0x06, 0x9f, 0xad, 0x34,
    0x20, 0x4e, 0x12, 0xf8, 0x62, 0x52, 0xef, 0xd0, 0x0e, 0xcf, 0x74, 0x4c, 0x0e, 0xe0, 0xfc, 0xd4,
];

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn main() -> Result<(), String> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| String::from("usage: exact_gnss_loss <output.ksb11>"))?;
    let mut session = FullMissionSession::new(OperationalRole::ScriptedOperator)
        .map_err(|error| format!("session create failed: {error:?}"))?;
    session
        .prepare()
        .map_err(|error| format!("session prepare failed: {error:?}"))?;
    let completed = session
        .run_scripted_to_completion()
        .map_err(|error| format!("session run failed: {error:?}"))?;
    let bundle = &completed.session.bundle;
    verify_complete_session(bundle)
        .map_err(|error| format!("completed KSB11 verification failed: {error:?}"))?;
    let digest = sha256(bundle);
    if completed.session.evidence.releases != 21_591
        || completed.session.evidence.actions.len() != 4
        || bundle.len() != EXPECTED_LENGTH
        || digest != EXPECTED_SHA256
    {
        return Err(String::from("exact accepted evidence identity mismatch"));
    }
    fs::write(&output, bundle).map_err(|error| format!("evidence write failed: {error}"))?;
    println!(
        "{{\"releaseEpoch\":21591,\"acceptedActions\":4,\"bytes\":{},\"sha256\":\"{}\"}}",
        bundle.len(),
        hex(&digest)
    );
    Ok(())
}
