use daedric_core::Manifest;
use std::path::PathBuf;

fn main() {
    let asar = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "/home/synth/Games/umu/umu-489830/drive_c/Program Files/DaedricOnline/resources/app.asar",
            )
        });
    match Manifest::from_asar(&asar) {
        Ok(m) => {
            println!("manifest decoded: {} tracked files", m.files.len());
            for (name, pin) in m.files.iter().take(5) {
                println!(
                    "  {} -> sha256 {} size {} entry {:?}",
                    name,
                    &pin.sha256[..16],
                    pin.size,
                    pin.entry
                );
            }
            if let Some(morphs) = m.files.get("RaceMenuMorphsCBBE.esp") {
                println!(
                    "field-check RaceMenuMorphsCBBE.esp: sha256={} size={}",
                    morphs.sha256, morphs.size
                );
            }
        }
        Err(e) => {
            eprintln!("decode failed: {e}");
            std::process::exit(1);
        }
    }
}
