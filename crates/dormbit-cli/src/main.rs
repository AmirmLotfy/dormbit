//! DORMBIT dev and test harness (subcommands arrive with Tasks 16-29).
//!
//! Takes paths as arguments because the developer running it is the trust
//! root. It is not part of the shipped app (ADR-005).

#![forbid(unsafe_code)]

fn main() {
    println!("dormbit-cli {}", dormbit_core::VERSION);
}
