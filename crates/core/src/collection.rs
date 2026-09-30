//! The three data contracts embedded in the launcher's `dist/main.js`.
//!
//! Verified against launcher 1.3.60 (asar re-probed 2026-09-29):
//! - `data/collection-lock.json` — schema/slug/revision + `mods[]` (240, archive
//!   md5/size pins) + `files{}` (242 per-plugin sha256 pins). NO `dlls` key in
//!   this build — the DLL allowlist is a separate module.
//! - `data/build-manifest.json` — `{_source, generatedAt, plugins{}, looseFiles{}}`;
//!   looseFiles are Data-relative paths (e.g. `interface/translations/racemenu_czech.txt`).
//! - `data/skse-dll-allowlist.json` — `{schema, generatedAt, overlayVersions, dlls{}}`;
//!   each DLL pins MULTIPLE accepted sha256 hashes.

use crate::extract::extract_module_json;
use crate::manifest::FilePin;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("module {0} not found in asar")]
    NotFound(&'static str),
    #[error("module {0} JSON parse failed: {1}")]
    Parse(&'static str, String),
}

fn parse_module<T: serde::de::DeserializeOwned>(
    text: &str,
    marker: &'static str,
) -> Result<T, ContractError> {
    let json = extract_module_json(text, marker).ok_or(ContractError::NotFound(marker))?;
    serde_json::from_str(&json).map_err(|e| ContractError::Parse(marker, e.to_string()))
}

// ---------- collection-lock ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPin {
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub mod_id: Option<u64>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub file_id: Option<u64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub archive_md5: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub archive_size: Option<u64>,
    #[serde(default)]
    pub archive_name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub fomod: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionLock {
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub revision: Option<u64>,
    #[serde(default)]
    pub file_set_sha256: Option<String>,
    #[serde(default)]
    pub mods: Vec<ModPin>,
    /// esp filename -> pin (same shape as manifest::FilePin)
    #[serde(default)]
    pub files: BTreeMap<String, FilePin>,
}

impl CollectionLock {
    pub fn from_asar(asar_path: &Path) -> Result<Self, ContractError> {
        let raw = std::fs::read(asar_path)?;
        let text = String::from_utf8_lossy(&raw);
        Self::from_asar_text(&text)
    }

    /// Parse from the asar contents already in memory (tests, recon dumps).
    pub fn from_asar_text(text: &str) -> Result<Self, ContractError> {
        parse_module(text, "data/collection-lock.json")
    }
}

// ---------- build-manifest ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeHash {
    pub sha256: String,
    #[serde(deserialize_with = "crate::de::u64_lenient")]
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildManifest {
    #[serde(default)]
    pub generated_at: Option<String>,
    /// plugin filename -> pin (the Play-gate hash set)
    #[serde(default)]
    pub plugins: BTreeMap<String, SizeHash>,
    /// Data-relative path -> pin (RaceMenu chargen swfs, translations)
    #[serde(default)]
    pub loose_files: BTreeMap<String, SizeHash>,
}

impl BuildManifest {
    pub fn from_asar(asar_path: &Path) -> Result<Self, ContractError> {
        let raw = std::fs::read(asar_path)?;
        let text = String::from_utf8_lossy(&raw);
        parse_module(&text, "data/build-manifest.json")
    }
}

// ---------- skse-dll-allowlist ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DllModRef {
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub mod_id: Option<u64>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub file_id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DllPin {
    /// Multiple accepted hashes (different valid builds of the same DLL).
    #[serde(default)]
    pub sha256: Vec<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub size: Option<u64>,
    #[serde(default)]
    pub mods: Vec<DllModRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DllAllowlist {
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub overlay_versions: Vec<String>,
    /// dll filename (lowercase as shipped) -> pin
    #[serde(default)]
    pub dlls: BTreeMap<String, DllPin>,
}

impl DllAllowlist {
    pub fn from_asar(asar_path: &Path) -> Result<Self, ContractError> {
        let raw = std::fs::read(asar_path)?;
        let text = String::from_utf8_lossy(&raw);
        parse_module(&text, "data/skse-dll-allowlist.json")
    }
}
