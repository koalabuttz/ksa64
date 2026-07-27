use std::process::Command;
fn git_output(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|x| x.status.success())
        .and_then(|x| String::from_utf8(x.stdout).ok())
        .map(|x| x.trim().to_owned())
}
fn main() {
    if let Some(head) = git_output(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(reference) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git_output(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(packed) = git_output(&["rev-parse", "--git-path", "packed-refs"]) {
        println!("cargo:rerun-if-changed={packed}");
    }
    let commit =
        git_output(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=KSA64_SOURCE_COMMIT={commit}");
    println!(
        "cargo:rustc-env=KSA64_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())
    );
}
