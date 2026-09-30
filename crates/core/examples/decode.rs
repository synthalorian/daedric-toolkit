use daedric_core::{Asar, BuildManifest, CollectionLock, DllAllowlist};
use std::path::PathBuf;

fn main() {
    let asar = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("DAEDRIC_ASAR").ok())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("usage: decode [asar_path]  (or set DAEDRIC_ASAR)");
            std::process::exit(2);
        });

    println!("== asar directory ==");
    match Asar::open(&asar) {
        Ok(a) => {
            for f in a.list() {
                println!("  {f}");
            }
            // real asar data files
            if let Ok(v) = a.read_json::<serde_json::Value>("data/modlist.json") {
                println!(
                    "  modlist.json: {} mods, rev {:?}",
                    v["mods"].as_array().map(|m| m.len()).unwrap_or(0),
                    v["collectionRevision"]
                );
            }
            if let Ok(v) = a.read_json::<serde_json::Value>("data/fomod-choices.json") {
                println!(
                    "  fomod-choices.json: {} mod entries",
                    v.as_object().map(|o| o.len()).unwrap_or(0)
                );
            }
        }
        Err(e) => eprintln!("  asar open failed: {e}"),
    }

    println!("\n== collection-lock (embedded) ==");
    match CollectionLock::from_asar(&asar) {
        Ok(lock) => {
            println!(
                "  {} rev {:?} — {} mods, {} file pins, fileSetSha256 {}",
                lock.slug.as_deref().unwrap_or("?"),
                lock.revision,
                lock.mods.len(),
                lock.files.len(),
                lock.file_set_sha256.as_deref().unwrap_or("?")
            );
            if let Some(cbbe) = lock.mods.iter().find(|m| m.mod_id == Some(30174)) {
                println!(
                    "  field-check CBBE 3BA: md5 {} size {}",
                    cbbe.archive_md5.as_deref().unwrap_or("?"),
                    cbbe.archive_size.unwrap_or(0)
                );
            }
        }
        Err(e) => eprintln!("  decode failed: {e}"),
    }

    println!("\n== build-manifest (embedded) ==");
    match BuildManifest::from_asar(&asar) {
        Ok(b) => println!(
            "  {} plugins, {} loose files (generatedAt {:?})",
            b.plugins.len(),
            b.loose_files.len(),
            b.generated_at
        ),
        Err(e) => eprintln!("  decode failed: {e}"),
    }

    println!("\n== skse-dll-allowlist (embedded) ==");
    match DllAllowlist::from_asar(&asar) {
        Ok(d) => println!(
            "  {} dlls, overlay versions {:?}",
            d.dlls.len(),
            d.overlay_versions
        ),
        Err(e) => eprintln!("  decode failed: {e}"),
    }
}
