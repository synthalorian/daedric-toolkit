use daedric_core::{Manifest, ScanReport};
use std::path::PathBuf;

/// Load the verification contract from a launcher app.asar.
#[tauri::command]
fn load_manifest(asar_path: String) -> Result<Manifest, String> {
    Manifest::from_asar(&PathBuf::from(asar_path)).map_err(|e| e.to_string())
}

/// Scan a Skyrim Data directory against the manifest.
#[tauri::command]
fn scan_data_dir(data_dir: String, asar_path: String) -> Result<ScanReport, String> {
    let manifest = Manifest::from_asar(&PathBuf::from(asar_path)).map_err(|e| e.to_string())?;
    daedric_core::scan_install(&PathBuf::from(data_dir), &manifest).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_manifest, scan_data_dir])
        .run(tauri::generate_context!())
        .expect("error while running Daedric Toolkit");
}
