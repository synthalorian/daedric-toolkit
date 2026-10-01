//! Game version, release book, and play-elsewhere against a live install.
//! Does not hash the Data dir.
//!
//! usage: play [asar_path] [skyrim_root]
//! or set DAEDRIC_ASAR and DAEDRIC_SKYRIM.

use daedric_core::{read_play_elsewhere, read_release_book, read_runtime, CollectionLock};
use std::path::PathBuf;

fn path_arg(nth: usize, env_key: &str) -> PathBuf {
    std::env::args()
        .nth(nth)
        .or_else(|| std::env::var(env_key).ok())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("usage: play [asar_path] [skyrim_root]  (or DAEDRIC_ASAR / DAEDRIC_SKYRIM)");
            std::process::exit(2);
        })
}

fn main() {
    let asar = path_arg(1, "DAEDRIC_ASAR");
    let root = path_arg(2, "DAEDRIC_SKYRIM");
    let accepted = CollectionLock::from_asar(&asar)
        .map(|lock| lock.game_versions)
        .unwrap_or_default();
    let runtime = read_runtime(&root, &accepted);
    println!(
        "runtime {} installed={:?} required={} loader={} failed={}",
        runtime.verdict,
        runtime.installed,
        runtime.required,
        runtime.loader_present,
        runtime.failed
    );
    println!("  {}", runtime.note);
    let release = read_release_book(&root);
    println!(
        "release {} servers={:?} failed={}",
        release.verdict, release.servers, release.failed
    );
    println!("  {}", release.note);
    let elsewhere = read_play_elsewhere(&root);
    println!(
        "elsewhere disabled={} parked={} failed={}",
        elsewhere.disabled, elsewhere.parked, elsewhere.failed
    );
    println!("  {}", elsewhere.note);
}
