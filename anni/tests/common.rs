use std::env;
use std::path::PathBuf;
use std::process::Command;

// https://github.com/rust-lang/cargo/blob/7fa132c7272fb9faca365c1d350e8e3c4c0d45e9/tests/cargotest/support/mod.rs#L316-L333
pub fn anni_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .map(|mut path| {
            path.pop();
            if path.ends_with("deps") {
                path.pop();
            }
            path
        })
        .expect("Cannot get anni_dir path.")
}

pub fn anni_exe() -> PathBuf {
    anni_dir().join(format!("anni{}", env::consts::EXE_SUFFIX))
}

pub fn run(subcommands: &[&str]) -> Command {
    let mut cmd = Command::new(anni_exe());
    cmd.args(subcommands);
    cmd
}
