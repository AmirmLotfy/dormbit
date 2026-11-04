//! dockline-gen: builds the fictional Dockline fixture (foundation Requirements 18 and 19).
//!
//!   cargo run -p dockline-gen --locked -- --out <dir>
//!
//! Writes `<dir>/dockline-invoices.tar` and `<dir>/dockline-invoices.sidecar.json` and nothing
//! else outside its own temp folder. The work runs in a child process whose `HOME` and
//! `XDG_CONFIG_HOME` point at an empty folder and whose `GIT_*` variables are cleared, so no
//! user or global Git configuration can leak into the history.

mod history;
mod pack;
mod sidecar;
mod stores;
mod time;
mod tree;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub type R<T> = Result<T, String>;

/// Adds context to an error and turns it into a message.
pub trait Ctx<T> {
    fn ctx(self, what: &str) -> R<T>;
}

impl<T, E: std::fmt::Display> Ctx<T> for Result<T, E> {
    fn ctx(self, what: &str) -> R<T> {
        self.map_err(|e| format!("{what}: {e}"))
    }
}

const TMP_ENV: &str = "DOCKLINE_GEN_TMP";
pub const TAR_NAME: &str = "dockline-invoices.tar";
pub const SIDECAR_NAME: &str = "dockline-invoices.sidecar.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(out) = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
    else {
        eprintln!("usage: dockline-gen --out <dir>");
        return ExitCode::from(2);
    };
    let out = PathBuf::from(out);
    let result = match std::env::var_os(TMP_ENV) {
        None => parent(&out),
        Some(tmp) => generate(Path::new(&tmp), &out),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dockline-gen: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Creates the temp folder, runs the child with a clean environment, and removes the folder.
fn parent(out: &Path) -> R<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("dockline-gen-{}-{stamp}", std::process::id()));
    let home = tmp.join("home");
    fs::create_dir_all(&home).ctx("temp folder")?;
    let mut cmd = Command::new(std::env::current_exe().ctx("current exe")?);
    cmd.arg("--out").arg(out);
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("GIT_") {
            cmd.env_remove(k);
        }
    }
    cmd.env("HOME", &home)
        .env("XDG_CONFIG_HOME", &home)
        .env(TMP_ENV, &tmp);
    let status = cmd.status().ctx("run the generator");
    let cleanup = fs::remove_dir_all(&tmp).ctx("remove the temp folder");
    match status?.success() {
        true => cleanup,
        false => Err("the generator failed".into()),
    }
}

fn generate(tmp: &Path, out: &Path) -> R<()> {
    let work = tmp.join("work");
    let project = work.join("dockline-invoices");
    history::build(&project)?;
    stores::build(&work.join("agent-stores"))?;
    let tar = pack::tar(&work)?;
    let sidecar = sidecar::build(&work, &project, &tar)?;

    // Temp names first, then rename, so a failed run never leaves a half-written fixture.
    fs::create_dir_all(out).ctx("output folder")?;
    let tar_tmp = out.join(format!(".{TAR_NAME}.partial"));
    let side_tmp = out.join(format!(".{SIDECAR_NAME}.partial"));
    fs::write(&tar_tmp, &tar).ctx("write tar")?;
    fs::write(&side_tmp, sidecar).ctx("write sidecar")?;
    fs::rename(&tar_tmp, out.join(TAR_NAME)).ctx("rename tar")?;
    fs::rename(&side_tmp, out.join(SIDECAR_NAME)).ctx("rename sidecar")
}
