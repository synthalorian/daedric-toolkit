//! The verification contract the launcher bundles in `app.asar`.
//!
//! Field-decoded schema (docs/the-mod-verification-gate.md):
//! - required mod list: modId, name, files[] { fileId, version, sizeBytes, optional, detect[] }
//! - canonical archive pin: archiveMd5, archiveSize, archiveName
//! - per-file hash map: "<name>.esp" -> { sha256, size, role, modId, fileId, entry }
//!
//! The asar body is plain text; the manifest is embedded as JSON inside the
//! bundled JS. Extraction is tolerant: locate the JSON region containing the
//! hash map, then parse.

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
pub struct FilePin {
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub modId: Option<u64>,
    #[serde(default)]
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
    /// Reality (recon 2026-09-29): the manifest is `data/collection-lock.json`,
    /// embedded in the esbuild bundle `dist/main.js` as a CommonJS module:
    /// `module2.exports = { ... files: { "3BBB.esp": { sha256: ..., size: ...,
    /// role: ..., modId: ..., fileId: ..., entry: ... }, ... } ... }` with
    /// UNQUOTED property keys (JS object literal, not strict JSON).
    ///
    /// Strategy: locate the collection-lock marker, find the `files: {` map
    /// after it, take the balanced object, quote bare keys, parse.
    pub fn from_asar(asar_path: &Path) -> Result<Self, ManifestError> {
        let raw = std::fs::read(asar_path)?;
        let text = String::from_utf8_lossy(&raw);
        extract_hash_map(&text).ok_or(ManifestError::NotFound)
    }

    /// Parse from an already-extracted JSON document (tests, recon output).
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let files: BTreeMap<String, FilePin> =
            serde_json::from_str(json).map_err(|e| ManifestError::Parse(e.to_string()))?;
        Ok(Manifest { files })
    }
}

/// Find the `files: { "<name>.esp": {...}, ... }` map inside the esbuild bundle.
///
/// Primary: anchor on the collection-lock module marker, then the first
/// `files: {` after it. Fallback: any `files: {` whose first key is an esp.
/// The extracted object has bare JS keys; quote them before serde_json.
fn extract_hash_map(text: &str) -> Option<Manifest> {
    let mut anchors: Vec<usize> = Vec::new();
    if let Some(marker) = text.find("data/collection-lock.json") {
        anchors.push(marker);
    }
    anchors.push(0); // fallback: scan the whole text for any `files: {` esp map

    for anchor in anchors {
        let mut search_from = anchor;
        while let Some(rel) = text[search_from..].find("files:") {
            let files_kw = search_from + rel;
            // find the opening brace of the map; if this occurrence has none,
            // skip it rather than aborting the whole search
            let brace = match text[files_kw..].find('{').map(|b| files_kw + b) {
                Some(b) => b,
                None => break,
            };
            // first key must look like an esp/esp/esm/esl filename
            let after = &text[brace + 1..];
            let trimmed = after.trim_start();
            let looks_like_plugin_map = trimmed
                .strip_prefix('"')
                .and_then(|rest| rest.find('"').map(|e| &rest[..e]))
                .map(|key| {
                    let k = key.to_ascii_lowercase();
                    k.ends_with(".esp") || k.ends_with(".esm") || k.ends_with(".esl")
                })
                .unwrap_or(false);
            if looks_like_plugin_map {
                if let Some(slice) = balanced_json_object(text.as_bytes(), brace) {
                    let quoted = quote_bare_keys(slice);
                    if let Ok(files) = serde_json::from_str::<BTreeMap<String, FilePin>>(&quoted) {
                        if !files.is_empty() {
                            return Some(Manifest { files });
                        }
                    }
                }
            }
            search_from = files_kw + 6;
        }
    }
    None
}

/// Quote bare JS-object-literal keys: `{ sha256: "..." }` -> `{ "sha256": "..." }`.
/// Only touches identifier keys after `{` or `,` outside strings.
fn quote_bare_keys(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + input.len() / 8);
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            out.push(b as char);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if (b == b'{' || b == b',') && !in_string {
            out.push(b as char);
            i += 1;
            // skip whitespace
            while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                out.push(bytes[i] as char);
                i += 1;
            }
            // bare identifier key followed by ':'?
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            if i > start {
                let mut j = i;
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b':' {
                    out.push('"');
                    out.push_str(&input[start..i]);
                    out.push('"');
                    continue; // whitespace before ':' emitted next loop
                } else {
                    out.push_str(&input[start..i]);
                    continue;
                }
            }
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

/// Given a byte offset at a `{`, return the balanced `{...}` slice as &str.
fn balanced_json_object(bytes: &[u8], start: usize) -> Option<&str> {
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    end.and_then(|e| std::str::from_utf8(&bytes[start..e]).ok())
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
    fn extracts_map_embedded_in_js() {
        // Real shape: CommonJS module, bare keys, esp map under `files:`.
        let js = "var require_lock = __commonJS({\n  \"data/collection-lock.json\"(exports2, module2) {\n    module2.exports = {\n      schema: \"daedric-collection/1\",\n      files: {\n        \"3BBB.esp\": {\n          sha256: \"a1aa2b0a08a1ce77ddb56ab48483f5a9978c671cf508d771e2a2b2f78fd0244e\",\n          size: 3288,\n          role: \"collection\",\n          modId: 30174,\n          fileId: 600100,\n          entry: \"10 Physics Patch/99 Only CBPC/3BBB.esp\"\n        },\n        \"[Kirax] Lost Ark Reborn Paladin Legendary.esp\": {\n          sha256: \"8c34899fd51edf3092421d266cad4443100d7205a1bbaa85e0579b7f1aea7da3\",\n          size: 16106,\n          role: \"overlay\",\n          modId: null,\n          fileId: null,\n          entry: null\n        }\n      }\n    };\n  }\n});";
        let m = extract_hash_map(js).unwrap();
        assert_eq!(m.files.len(), 2);
        let pin = &m.files["3BBB.esp"];
        assert_eq!(pin.size, 3288);
        assert_eq!(pin.modId, Some(30174));
        // null fields tolerated
        let kirax = &m.files["[Kirax] Lost Ark Reborn Paladin Legendary.esp"];
        assert_eq!(kirax.modId, None);
    }
}
