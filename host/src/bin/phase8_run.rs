use std::{env, fs};

fn main() {
    let evidence = ksa64_host::phase8::run_checked_in_phase8()
        .unwrap_or_else(|error| panic!("run Phase 8 mission: {error:?}"));
    let json = serde_json::to_vec_pretty(&evidence).expect("serialize Phase 8 evidence");
    if let Some(path) = env::args_os().nth(1) {
        fs::write(path, &json).expect("write Phase 8 evidence");
    } else {
        println!("{}", String::from_utf8(json).expect("JSON is UTF-8"));
    }
}
