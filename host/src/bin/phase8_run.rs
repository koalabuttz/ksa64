use std::{env, ffi::OsStr, fs};

fn main() {
    let mut arguments = env::args_os().skip(1);
    let output = arguments.next();
    let case = arguments.next();
    if arguments.next().is_some() {
        eprintln!("usage: phase8_run [OUTPUT] [calm|crosswind5]");
        std::process::exit(2);
    }
    let evidence = match case.as_deref() {
        None => ksa64_host::phase8::run_checked_in_phase8(),
        Some(value) if value == OsStr::new("calm") => ksa64_host::phase8::run_checked_in_phase8(),
        Some(value) if value == OsStr::new("crosswind5") => {
            ksa64_host::phase8::run_checked_in_phase8_crosswind(5)
        }
        Some(_) => {
            eprintln!("unknown Phase 8 case; expected calm or crosswind5");
            std::process::exit(2);
        }
    }
    .unwrap_or_else(|error| panic!("run Phase 8 mission: {error:?}"));
    let json = serde_json::to_vec_pretty(&evidence).expect("serialize Phase 8 evidence");
    if let Some(path) = output {
        fs::write(path, &json).expect("write Phase 8 evidence");
    } else {
        println!("{}", String::from_utf8(json).expect("JSON is UTF-8"));
    }
}
