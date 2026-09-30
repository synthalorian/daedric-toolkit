//! Minimal read-only asar archive parser.
//!
//! Layout (recon §1, verified against launcher 1.3.60):
//! 4×u32-LE header `[4, header_block_size, header_string_size, json_length]`,
//! JSON directory at byte 16, file data starting at `8 + header_block_size`.
//! Directory entries store `offset` (relative to data start) and `size` as
//! DECIMAL STRINGS. Never hardcode offsets — they shift per launcher release.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum AsarError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad asar header")]
    BadHeader,
    #[error("directory JSON parse failed: {0}")]
    Parse(String),
    #[error("file not in archive: {0}")]
    NotFound(String),
}

#[derive(Debug, Deserialize)]
struct DirEntry {
    /// asar spec says decimal strings; real builds mix ints in — be lenient.
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    size: Option<u64>,
    #[serde(default, deserialize_with = "crate::de::opt_u64_lenient")]
    offset: Option<u64>,
    #[serde(default)]
    files: Option<BTreeMap<String, DirEntry>>,
}

pub struct Asar {
    raw: Vec<u8>,
    data_start: usize,
    root: DirEntry,
}

impl Asar {
    pub fn open(path: &Path) -> Result<Self, AsarError> {
        let raw = std::fs::read(path)?;
        if raw.len() < 16 {
            return Err(AsarError::BadHeader);
        }
        let u32le = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap()) as usize;
        let header_string_size = u32le(8);
        let json_length = u32le(12);
        let header_block_size = u32le(4);
        if u32le(0) != 4 || 16 + json_length > raw.len() {
            return Err(AsarError::BadHeader);
        }
        let json = std::str::from_utf8(&raw[16..16 + json_length])
            .map_err(|e| AsarError::Parse(e.to_string()))?;
        let root: DirEntry =
            serde_json::from_str(json).map_err(|e| AsarError::Parse(e.to_string()))?;
        // data starts after the 8-byte prefix + the whole header block;
        // header_string_size is the authoritative JSON region size
        let _ = header_string_size;
        Ok(Self {
            raw,
            data_start: 8 + header_block_size,
            root,
        })
    }

    /// List all file paths in the archive (e.g. `data/modlist.json`).
    pub fn list(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(files) = &self.root.files {
            walk(files, "", &mut out);
        }
        out
    }

    /// Read a file by path (`data/modlist.json`; leading slash tolerated).
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, AsarError> {
        let path = path.trim_start_matches('/');
        let mut node = &self.root;
        for part in path.split('/') {
            let files = node
                .files
                .as_ref()
                .ok_or_else(|| AsarError::NotFound(path.into()))?;
            node = files
                .get(part)
                .ok_or_else(|| AsarError::NotFound(path.into()))?;
        }
        let offset = node
            .offset
            .ok_or_else(|| AsarError::NotFound(path.into()))? as usize;
        let size = node.size.ok_or_else(|| AsarError::NotFound(path.into()))? as usize;
        let start = self.data_start + offset;
        let end = start + size;
        if end > self.raw.len() {
            return Err(AsarError::BadHeader);
        }
        Ok(self.raw[start..end].to_vec())
    }

    /// Read a file and parse it as JSON.
    pub fn read_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, AsarError> {
        let bytes = self.read_file(path)?;
        serde_json::from_slice(&bytes).map_err(|e| AsarError::Parse(e.to_string()))
    }
}

fn walk(files: &BTreeMap<String, DirEntry>, prefix: &str, out: &mut Vec<String>) {
    for (name, entry) in files {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if let Some(children) = &entry.files {
            walk(children, &path, out);
        } else {
            out.push(path);
        }
    }
}
