use daedric_core::{
    BuildGateReport, BuildManifest, CollectionLock, DllAllowlist, DownloadsReport, Manifest,
    OverlayReport, ScanReport,
};
use serde::Serialize;
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

/// Decoded-contract summary: the counts and identity fields from all three
/// data modules embedded in the launcher's asar.
#[derive(Debug, Clone, Serialize)]
pub struct ContractSummary {
    pub mods: usize,
    pub file_pins: usize,
    pub plugins: usize,
    pub loose_files: usize,
    pub dlls: usize,
    pub slug: Option<String>,
    pub revision: Option<u64>,
    pub file_set_sha256: Option<String>,
}

fn summarize(lock: &CollectionLock, build: &BuildManifest, dlls: &DllAllowlist) -> ContractSummary {
    ContractSummary {
        mods: lock.mods.len(),
        file_pins: lock.files.len(),
        plugins: build.plugins.len(),
        loose_files: build.loose_files.len(),
        dlls: dlls.dlls.len(),
        slug: lock.slug.clone(),
        revision: lock.revision,
        file_set_sha256: lock.file_set_sha256.clone(),
    }
}

/// Decode all three embedded contracts and return just the summary strip.
#[tauri::command]
fn decode_contracts(asar_path: String) -> Result<ContractSummary, String> {
    let asar = PathBuf::from(asar_path);
    let lock = CollectionLock::from_asar(&asar).map_err(|e| e.to_string())?;
    let build = BuildManifest::from_asar(&asar).map_err(|e| e.to_string())?;
    let dlls = DllAllowlist::from_asar(&asar).map_err(|e| e.to_string())?;
    Ok(summarize(&lock, &build, &dlls))
}

/// The esp gate: every pinned plugin checked against the Data dir.
#[tauri::command]
fn scan_esp_gate(data_dir: String, asar_path: String) -> Result<ScanReport, String> {
    let manifest = Manifest::from_asar(&PathBuf::from(asar_path)).map_err(|e| e.to_string())?;
    daedric_core::scan_install(&PathBuf::from(data_dir), &manifest).map_err(|e| e.to_string())
}

/// The Play-gate: build-manifest plugins + loose files + SKSE DLL allowlist.
#[tauri::command]
fn scan_build_gate(data_dir: String, asar_path: String) -> Result<BuildGateReport, String> {
    let asar = PathBuf::from(asar_path);
    let build = BuildManifest::from_asar(&asar).map_err(|e| e.to_string())?;
    let dlls = DllAllowlist::from_asar(&asar).map_err(|e| e.to_string())?;
    daedric_core::scan_build_gate(&PathBuf::from(data_dir), &build, &dlls)
        .map_err(|e| e.to_string())
}

/// Verify downloaded mod archives (`<skyrim_root>/DaedricData/downloads`).
#[tauri::command]
fn verify_downloads(skyrim_root: String, asar_path: String) -> Result<DownloadsReport, String> {
    let lock = CollectionLock::from_asar(&PathBuf::from(asar_path)).map_err(|e| e.to_string())?;
    let downloads = PathBuf::from(skyrim_root)
        .join("DaedricData")
        .join("downloads");
    daedric_core::verify_downloads(&downloads, &lock).map_err(|e| e.to_string())
}

/// Overlay drift check: installed overlay marker vs the contract's accepted set.
/// Drift means "the launcher will push an update at next Play" — gate failures
/// on a drifting install are version skew, not corruption.
#[tauri::command]
fn check_overlay_drift(skyrim_root: String, asar_path: String) -> Result<OverlayReport, String> {
    let dlls = DllAllowlist::from_asar(&PathBuf::from(asar_path)).map_err(|e| e.to_string())?;
    Ok(daedric_core::check_overlay(
        &PathBuf::from(skyrim_root),
        &dlls,
    ))
}

/// Everything the doctor reports in one run.
#[derive(Debug, Clone, Serialize)]
pub struct FullDoctorReport {
    pub esp: ScanReport,
    pub gate: BuildGateReport,
    pub downloads: DownloadsReport,
    pub contracts: ContractSummary,
    pub overlay: OverlayReport,
}

/// Full install-doctor sweep: decode contracts, scan the esp gate, the
/// build gate, and the downloads folder.
#[tauri::command]
fn run_full_doctor(skyrim_root: String, asar_path: String) -> Result<FullDoctorReport, String> {
    let asar = PathBuf::from(asar_path);
    let root = PathBuf::from(skyrim_root);
    let data = root.join("Data");

    let lock = CollectionLock::from_asar(&asar).map_err(|e| e.to_string())?;
    let build = BuildManifest::from_asar(&asar).map_err(|e| e.to_string())?;
    let dlls = DllAllowlist::from_asar(&asar).map_err(|e| e.to_string())?;

    let contracts = summarize(&lock, &build, &dlls);
    let overlay = daedric_core::check_overlay(&root, &dlls);

    let manifest = Manifest {
        files: lock.files.clone(),
    };
    let esp = daedric_core::scan_install(&data, &manifest).map_err(|e| e.to_string())?;
    let gate = daedric_core::scan_build_gate(&data, &build, &dlls).map_err(|e| e.to_string())?;
    let downloads =
        daedric_core::verify_downloads(&root.join("DaedricData").join("downloads"), &lock)
            .map_err(|e| e.to_string())?;

    Ok(FullDoctorReport {
        esp,
        gate,
        downloads,
        contracts,
        overlay,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_manifest,
            scan_data_dir,
            decode_contracts,
            scan_esp_gate,
            scan_build_gate,
            verify_downloads,
            check_overlay_drift,
            run_full_doctor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Daedric Toolkit");
}
