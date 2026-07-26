use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn binary(name: &str) -> PathBuf {
    match name {
        "ksa64" => PathBuf::from(env!("CARGO_BIN_EXE_ksa64")),
        "ksa64-host" => PathBuf::from(env!("CARGO_BIN_EXE_ksa64-host")),
        "phase11" => PathBuf::from(env!("CARGO_BIN_EXE_phase11")),
        _ => panic!("unknown test binary"),
    }
}

fn run(name: &str, arguments: &[&str]) -> Output {
    Command::new(binary(name))
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("run {name}: {error}"))
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ksa64-phase11-5-{}-{}", std::process::id(), name))
}

#[test]
fn quick_start_and_catalog_are_deterministic() {
    let first = run("ksa64", &[]);
    let second = run("ksa64", &[]);
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let text = String::from_utf8(first.stdout).unwrap();
    assert!(text.contains("mission control ksa-g10r.operations --scenario gnss-loss"));
    assert!(text.contains("Nothing above launches hardware or VICE implicitly"));

    let first = run("ksa64", &["--json", "catalog", "list", "--historical"]);
    let second = run("ksa64", &["--json", "catalog", "list", "--historical"]);
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let value: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["schema"], "ksa64.product-catalog.v1");
    assert_eq!(value["experiences"].as_array().unwrap().len(), 13);
}

#[test]
fn phase11_wrapper_and_unified_project_produce_identical_sessions() {
    let unified = temp("unified.ksb11");
    let legacy = temp("legacy.ksb11");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("phase11/examples/gnss-loss.json");

    let unified_output = Command::new(binary("ksa64"))
        .args(["project", "script"])
        .arg(&source)
        .arg("--output")
        .arg(&unified)
        .output()
        .unwrap();
    assert!(
        unified_output.status.success(),
        "{}",
        String::from_utf8_lossy(&unified_output.stderr)
    );
    let legacy_output = Command::new(binary("phase11"))
        .arg("script")
        .arg(&source)
        .arg(&legacy)
        .output()
        .unwrap();
    assert!(
        legacy_output.status.success(),
        "{}",
        String::from_utf8_lossy(&legacy_output.stderr)
    );
    assert_eq!(fs::read(&unified).unwrap(), fs::read(&legacy).unwrap());
    assert_eq!(
        String::from_utf8(legacy_output.stdout).unwrap(),
        "completed evidence 0x6d4122a0 (22369 bytes)\n"
    );

    let _ = fs::remove_file(unified);
    let _ = fs::remove_file(legacy);
}

#[test]
fn ksa64_host_keeps_original_capture_aliases() {
    let unified = temp("capture-new.kst");
    let alias = temp("capture-alias.kst");
    let a = Command::new(binary("ksa64"))
        .arg("capture")
        .arg(&unified)
        .output()
        .unwrap();
    let b = Command::new(binary("ksa64-host"))
        .arg("capture")
        .arg(&alias)
        .output()
        .unwrap();
    assert!(a.status.success());
    assert!(b.status.success());
    assert_eq!(a.stdout, b.stdout);
    let normalized = String::from_utf8(a.stderr)
        .unwrap()
        .replace(unified.to_str().unwrap(), alias.to_str().unwrap());
    assert_eq!(normalized.as_bytes(), b.stderr);
    assert_eq!(fs::read(&unified).unwrap(), fs::read(&alias).unwrap());

    let _ = fs::remove_file(unified);
    let _ = fs::remove_file(alias);
}

#[test]
fn target_verification_is_stored_and_live_probe_is_explicit() {
    let stored = run("ksa64", &["target", "verify", "c64.ksa-g10r.reference-ops"]);
    assert!(stored.status.success());
    assert!(String::from_utf8(stored.stdout)
        .unwrap()
        .contains("no emulator launched"));

    let refused = run("ksa64", &["target", "probe", "c64.ksa-g10r.reference-ops"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8(refused.stderr)
        .unwrap()
        .contains("target.live-required"));
}
