//! The verification contract the launcher bundles in `app.asar`.
//!
//! Field-decoded schema (docs/the-mod-verification-gate.md):
//! - required mod list: modId, name, files[] { fileId, version, sizeBytes, optional, detect[] }
//! - canonical archive pin: archiveMd5, archiveSize, archiveName
//! - per-file hash map: "<name>.esp" -> { sha256, size, role, modId, fileId, entry }
//!
//! The per-file map lives in the `data/collection-lock.json` esbuild module
//! (JS object literal, bare keys) — decoded via `crate::collection::CollectionLock`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("no manifest found in asar (looked for per-file sha256 map)")]
    NotFound,
    #[error("manifest JSON parse failed: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct FilePin {
    pub sha256: String,
    #[serde(deserialize_with = "crate::de::u64_lenient")]
    pub size: u64,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub modId: Option<u64>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    pub fileId: Option<u64>,
    /// Exact installer-option path inside the canonical archive whose bytes are canonical.
    #[serde(default)]
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    /// esp filename -> pin
    #[serde(default)]
    pub files: BTreeMap<String, FilePin>,
}

impl Manifest {
    /// Extract the per-file hash map from a launcher `app.asar`.
    ///
    /// Reality (re-probed 2026-09-29): the map is the `files:` key of the
    /// `data/collection-lock.json` CommonJS module embedded in `dist/main.js`.
    pub fn from_asar(asar_path: &Path) -> Result<Self, ManifestError> {
        let lock =
            crate::collection::CollectionLock::from_asar(asar_path).map_err(|e| match e {
                crate::collection::ContractError::Io(io) => ManifestError::Io(io),
                crate::collection::ContractError::NotFound(_) => ManifestError::NotFound,
                crate::collection::ContractError::Parse(_, m) => ManifestError::Parse(m),
            })?;
        if lock.files.is_empty() {
            return Err(ManifestError::NotFound);
        }
        Ok(Manifest { files: lock.files })
    }

    /// Parse from an already-extracted JSON document (tests, recon output).
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let files: BTreeMap<String, FilePin> =
            serde_json::from_str(json).map_err(|e| ManifestError::Parse(e.to_string()))?;
        Ok(Manifest { files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{"3BBB.esp":{"sha256":"a1aa2b0a08a1ce77ddb56ab48483f5a9978c671cf508d771e2a2b2f78fd0244e","size":3288,"role":"collection","modId":30174,"fileId":600100,"entry":"10 Physics Patch/99 Only CBPC/3BBB.esp"},"RaceMenuMorphsCBBE.esp":{"sha256":"1abd6b7176b16a479bd2ea8fc8844c622588083eef7fc3e810a5aa1f62014d29","size":697,"role":"collection","modId":30174,"fileId":600100,"entry":"15 RaceMenuMorphs/00 RaceMenuMorphs - CBBE/RaceMenuMorphsCBBE.esp"}}"##;

    #[test]
    fn parses_field_manifest() {
        let m = Manifest::from_json(SAMPLE).unwrap();
        assert_eq!(m.files.len(), 2);
        let morphs = &m.files["RaceMenuMorphsCBBE.esp"];
        assert_eq!(morphs.size, 697);
        assert_eq!(
            morphs.entry.as_deref(),
            Some("15 RaceMenuMorphs/00 RaceMenuMorphs - CBBE/RaceMenuMorphsCBBE.esp")
        );
    }

    #[test]
    fn extracts_lock_embedded_in_js() {
        // Real shape: CommonJS module, bare keys, esp map under `files:`.
        let js = "var require_lock = __commonJS({\n  \"data/collection-lock.json\"(exports2, module2) {\n    module2.exports = {\n      schema: \"daedric-collection/1\",\n      files: {\n        \"3BBB.esp\": {\n          sha256: \"a1aa2b0a08a1ce77ddb56ab48483f5a9978c671cf508d771e2a2b2f78fd0244e\",\n          size: 3288,\n          role: \"collection\",\n          modId: 30174,\n          fileId: 600100,\n          entry: \"10 Physics Patch/99 Only CBPC/3BBB.esp\"\n        },\n        \"[Kirax] Lost Ark Reborn Paladin Legendary.esp\": {\n          sha256: \"8c34899fd51edf3092421d266cad4443100d7205a1bbaa85e0579b7f1aea7da3\",\n          size: 16106,\n          role: \"overlay\",\n          modId: null,\n          fileId: null,\n          entry: null\n        }\n      }\n    };\n  }\n});";
        let lock = crate::collection::CollectionLock::from_asar_text(js).unwrap();
        assert_eq!(lock.files.len(), 2);
        let pin = &lock.files["3BBB.esp"];
        assert_eq!(pin.size, 3288);
        assert_eq!(pin.modId, Some(30174));
        // null fields tolerated
        let kirax = &lock.files["[Kirax] Lost Ark Reborn Paladin Legendary.esp"];
        assert_eq!(kirax.modId, None);
    }
}
