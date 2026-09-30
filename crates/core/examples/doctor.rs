//! Full install-doctor run against a live Skyrim SE + Daedric Online install.
//!
//! usage: doctor [asar_path] [skyrim_root]
//! defaults: synth's field machine.

use daedric_core::{
    scan_build_gate, scan_install, verify_downloads, BuildManifest, CollectionLock, DllAllowlist,
    Manifest,
};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let asar = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(
            "/home/synth/Games/umu/umu-489830/drive_c/Program Files/DaedricOnline/resources/app.asar",
        )
    });
    let root = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/home/synth/.local/share/Steam/steamapps/common/Skyrim Special Edition")
        });
    let data = root.join("Data");

    println!("== decode contracts ==");
    let lock = CollectionLock::from_asar(&asar).expect("collection-lock decode");
    let build = BuildManifest::from_asar(&asar).expect("build-manifest decode");
    let dlls = DllAllowlist::from_asar(&asar).expect("dll allowlist decode");
    let manifest = Manifest {
        files: lock.files.clone(),
    };
    println!(
        "  collection-lock: {} mods / {} file pins | build-manifest: {} plugins + {} loose | dlls: {}",
        lock.mods.len(),
        lock.files.len(),
        build.plugins.len(),
        build.loose_files.len(),
        dlls.dlls.len()
    );

    println!("\n== esp gate (collection-lock files:) ==");
    let t = Instant::now();
    let report = scan_install(&data, &manifest).expect("esp scan");
    println!(
        "  {} ok / {} failed in {:?} — gate {}",
        report.ok,
        report.failed,
        t.elapsed(),
        if report.passes_gate() {
            "PASSES"
        } else {
            "FAILS"
        }
    );
    for f in report
        .files
        .iter()
        .filter(|f| f.path.is_none() || !matches!(f.verdict, daedric_core::FileVerdict::Ok))
    {
        println!("    FAIL {:?} {}", f.verdict, f.esp);
    }

    println!("\n== build gate (plugins + loose + SKSE dlls) ==");
    let t = Instant::now();
    let gate = scan_build_gate(&data, &build, &dlls).expect("build gate scan");
    println!(
        "  {} ok / {} failed in {:?} — gate {}",
        gate.ok,
        gate.failed,
        t.elapsed(),
        if gate.passes_gate() {
            "PASSES"
        } else {
            "FAILS"
        }
    );
    for f in gate
        .files
        .iter()
        .take(20)
        .filter(|f| !matches!(f.verdict, daedric_core::verify::GateVerdict::Ok))
    {
        println!("    FAIL [{}] {:?} {}", f.kind, f.verdict, f.file);
    }

    println!("\n== downloads (archive md5 pins) ==");
    let dl = root.join("DaedricData").join("downloads");
    match verify_downloads(&dl, &lock) {
        Ok(rep) => {
            println!(
                "  {} ok / {} failed in {}",
                rep.ok,
                rep.failed,
                dl.display()
            );
            for a in rep.archives.iter().take(10).filter(|a| {
                !matches!(
                    a.verdict,
                    daedric_core::verify::ArchiveVerdict::Ok
                        | daedric_core::verify::ArchiveVerdict::Unpinned
                )
            }) {
                println!(
                    "    {:?} {} ({:?}/{:?})",
                    a.verdict, a.name, a.mod_id, a.file_id
                );
            }
        }
        Err(e) => println!("  skipped: {e}"),
    }
}
