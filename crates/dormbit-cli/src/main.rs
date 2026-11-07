//! DORMBIT dev and test harness (subcommands arrive with Tasks 16-29).
//!
//! Takes paths as arguments because the developer running it is the trust
//! root. It is not part of the shipped app (ADR-005).
#![forbid(unsafe_code)]

mod capture;
mod plan;
mod verify;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("plan") => plan::run(&args[1..]),
        Some("capture") => capture::run(&args[1..]),
        Some("verify") => verify::run(&args[1..]),
        Some("version") | None => Ok(format!("dormbit-cli {}\n", dormbit_core::VERSION)),
        Some(other) => Err(format!(
            "unknown command: {other} (try: plan, capture, verify, version)"
        )),
    };
    match result {
        Ok(text) => print!("{text}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
