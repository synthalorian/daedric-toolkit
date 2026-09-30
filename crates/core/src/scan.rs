//! Scan a real install against the manifest contract.
//!
//! The gate only checks esp bytes — meshes, textures, BSAs are unverified.
//! Data dir layout differs per platform, but the check is the same:
//! for each pinned esp, find it under the Data dir, hash it, compare.

use crate::hash::sha256_file;
use crate::manifest::Manifest;
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub enum FileVerdict {
    /// Bytes match the server pin exactly.
    Ok,
    /// File exists but bytes differ ("was edited" / "different version").
    HashMismatch {
        expected: String,
        actual: String,
        actual_size: u64,
    },
    /// File size differs (fast pre-check failure).
    SizeMismatch { expected: u64, actual: u64 },
    /// Tracked esp not present anywhere under Data.
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileReport {
    pub esp: String,
    pub verdict: FileVerdict,
    pub path: Option<PathBuf>,
    /// Canonical archive entry that holds the correct bytes.
    pub canonical_entry: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub data_dir: PathBuf,
    pub files: Vec<FileReport>,
    pub ok: usize,
    pub failed: usize,
}

impl ScanReport {
    pub fn passes_gate(&self) -> bool {
        self.failed == 0
    }
}

/// Verify every file the manifest tracks against a Data directory.
///
/// Two entry shapes live in the collection-lock `files:` map:
/// - plugin basenames (`3BBB.esp`) — searched anywhere under Data
/// - Data-relative paths (`interface/racemenu/bottombar.swf`) — checked in place
pub fn scan_install(data_dir: &Path, manifest: &Manifest) -> std::io::Result<ScanReport> {
    // Index plugin files by lowercase basename (Wine/NTFS semantics: case-insensitive).
    let mut index: std::collections::HashMap<String, PathBuf> = Default::default();
    for entry in WalkDir::new(data_dir).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".esp") || lower.ends_with(".esm") || lower.ends_with(".esl") {
                    index
                        .entry(lower)
                        .or_insert_with(|| entry.path().to_path_buf());
                }
            }
        }
    }

    let mut files = Vec::new();
    let mut ok = 0usize;
    let mut failed = 0usize;

    for (esp, pin) in &manifest.files {
        // Data-relative entries (contain a path separator) check in place;
        // bare plugin names resolve via the walk index.
        let is_relative_path = esp.contains('/') || esp.contains('\\');
        let found = if is_relative_path {
            let p = data_dir.join(esp.replace('/', std::path::MAIN_SEPARATOR_STR));
            if p.is_file() {
                Some(p)
            } else {
                find_case_insensitive_path(data_dir, esp)
            }
        } else {
            index.get(&esp.to_ascii_lowercase()).cloned()
        };
        let (verdict, path) = match found {
            None => (FileVerdict::Missing, None),
            Some(p) => {
                let meta = std::fs::metadata(&p)?;
                let verdict = if meta.len() != pin.size {
                    FileVerdict::SizeMismatch {
                        expected: pin.size,
                        actual: meta.len(),
                    }
                } else {
                    let actual = sha256_file(&p).map_err(std::io::Error::other)?;
                    if actual == pin.sha256 {
                        FileVerdict::Ok
                    } else {
                        FileVerdict::HashMismatch {
                            expected: pin.sha256.clone(),
                            actual,
                            actual_size: meta.len(),
                        }
                    }
                };
                (verdict, Some(p))
            }
        };
        if matches!(verdict, FileVerdict::Ok) {
            ok += 1;
        } else {
            failed += 1;
        }
        files.push(FileReport {
            esp: esp.clone(),
            verdict,
            path,
            canonical_entry: pin.entry.clone(),
        });
    }

    // Stable output: failures first, then alphabetical.
    files.sort_by(|a, b| {
        let af = !matches!(a.verdict, FileVerdict::Ok);
        let bf = !matches!(b.verdict, FileVerdict::Ok);
        bf.cmp(&af).then_with(|| a.esp.cmp(&b.esp))
    });

    Ok(ScanReport {
        data_dir: data_dir.to_path_buf(),
        files,
        ok,
        failed,
    })
}

/// Resolve a Data-relative path case-insensitively, component by component
/// (Wine/NTFS semantics on a case-sensitive Linux fs).
fn find_case_insensitive_path(data_dir: &Path, rel: &str) -> Option<PathBuf> {
    let mut cur = data_dir.to_path_buf();
    for part in rel.split(['/', '\\']) {
        let direct = cur.join(part);
        if direct.exists() {
            cur = direct;
            continue;
        }
        let want = part.to_ascii_lowercase();
        let next = std::fs::read_dir(&cur).ok()?.flatten().find(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.to_ascii_lowercase() == want)
                .unwrap_or(false)
        });
        cur = next?.path();
    }
    if cur.is_file() {
        Some(cur)
    } else {
        None
    }
}
