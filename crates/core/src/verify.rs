//! Verify the parts of an install the esp-gate doesn't cover:
//! downloaded archives (`DaedricData/downloads`), the build-manifest loose
//! files, and the SKSE DLL allowlist.

use crate::collection::{BuildManifest, CollectionLock, DllAllowlist};
use crate::hash::{md5_file, sha256_file};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub enum ArchiveVerdict {
    /// md5 + size match the collection pin.
    Ok,
    HashMismatch {
        expected: String,
        actual: String,
    },
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    /// Archive not found in downloads (looked for `<modId>-<fileId>.archive` and the canonical name).
    Missing,
    /// Pin has no md5 to check against.
    Unpinned,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveReport {
    pub mod_id: Option<u64>,
    pub file_id: Option<u64>,
    pub name: String,
    pub category: Option<String>,
    pub verdict: ArchiveVerdict,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadsReport {
    pub downloads_dir: PathBuf,
    pub archives: Vec<ArchiveReport>,
    pub ok: usize,
    pub failed: usize,
}

/// Verify downloaded mod archives against the collection-lock pins.
///
/// Archive naming (recon §3): the launcher stores downloads as
/// `<modId>-<fileId>.archive`; Mod Manager Download drops may keep the
/// original Nexus filename. Both are tried.
pub fn verify_downloads(
    downloads_dir: &Path,
    lock: &CollectionLock,
) -> std::io::Result<DownloadsReport> {
    let mut archives = Vec::new();
    let mut ok = 0usize;
    let mut failed = 0usize;

    for m in &lock.mods {
        // skip non-installable categories
        let category = m.category.clone();
        if matches!(category.as_deref(), Some("OLD_VERSION") | Some("ARCHIVED")) {
            continue;
        }

        let candidates: Vec<PathBuf> = match (m.mod_id, m.file_id) {
            (Some(mid), Some(fid)) => vec![
                downloads_dir.join(format!("{mid}-{fid}.archive")),
                downloads_dir.join(format!("{mid}-{fid}")),
            ],
            _ => vec![],
        };
        let mut found = candidates.into_iter().find(|p| p.is_file());
        if found.is_none() {
            if let Some(name) = &m.archive_name {
                let p = downloads_dir.join(name);
                if p.is_file() {
                    found = Some(p);
                }
            }
        }

        let (verdict, path) = match found {
            None => (ArchiveVerdict::Missing, None),
            Some(p) => {
                let meta = std::fs::metadata(&p)?;
                let verdict = match (&m.archive_md5, m.archive_size) {
                    (Some(exp_md5), Some(exp_size)) => {
                        if meta.len() != exp_size {
                            ArchiveVerdict::SizeMismatch {
                                expected: exp_size,
                                actual: meta.len(),
                            }
                        } else {
                            let actual = md5_file(&p).map_err(std::io::Error::other)?;
                            if actual == *exp_md5 {
                                ArchiveVerdict::Ok
                            } else {
                                ArchiveVerdict::HashMismatch {
                                    expected: exp_md5.clone(),
                                    actual,
                                }
                            }
                        }
                    }
                    _ => ArchiveVerdict::Unpinned,
                };
                (verdict, Some(p))
            }
        };
        if matches!(verdict, ArchiveVerdict::Ok | ArchiveVerdict::Unpinned) {
            ok += 1;
        } else {
            failed += 1;
        }
        archives.push(ArchiveReport {
            mod_id: m.mod_id,
            file_id: m.file_id,
            name: m.name.clone(),
            category,
            verdict,
            path,
        });
    }

    archives.sort_by(|a, b| {
        let af = !matches!(a.verdict, ArchiveVerdict::Ok | ArchiveVerdict::Unpinned);
        let bf = !matches!(b.verdict, ArchiveVerdict::Ok | ArchiveVerdict::Unpinned);
        bf.cmp(&af).then_with(|| a.name.cmp(&b.name))
    });

    Ok(DownloadsReport {
        downloads_dir: downloads_dir.to_path_buf(),
        archives,
        ok,
        failed,
    })
}

#[derive(Debug, Clone, Serialize)]
pub enum GateVerdict {
    Ok,
    HashMismatch { expected: String, actual: String },
    SizeMismatch { expected: u64, actual: u64 },
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateFileReport {
    /// Plugin name or Data-relative path.
    pub file: String,
    pub kind: &'static str, // "plugin" | "loose" | "dll"
    pub verdict: GateVerdict,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildGateReport {
    pub data_dir: PathBuf,
    pub files: Vec<GateFileReport>,
    pub ok: usize,
    pub failed: usize,
}

impl BuildGateReport {
    pub fn passes_gate(&self) -> bool {
        self.failed == 0
    }
}

/// The Play-gate check: build-manifest plugins + loose files + SKSE DLL
/// allowlist, all against the Data dir. This is what the launcher enforces
/// at Play — passing this means the launcher lets you in.
pub fn scan_build_gate(
    data_dir: &Path,
    build: &BuildManifest,
    dlls: &DllAllowlist,
) -> std::io::Result<BuildGateReport> {
    let mut files = Vec::new();

    let mut check = |path: PathBuf,
                     file: String,
                     kind: &'static str,
                     expected_sha: &str,
                     expected_size: u64| {
        let verdict = match std::fs::metadata(&path) {
            Err(_) => GateVerdict::Missing,
            Ok(meta) => {
                if meta.len() != expected_size {
                    GateVerdict::SizeMismatch {
                        expected: expected_size,
                        actual: meta.len(),
                    }
                } else {
                    match sha256_file(&path) {
                        Ok(actual) if actual == expected_sha => GateVerdict::Ok,
                        Ok(actual) => GateVerdict::HashMismatch {
                            expected: expected_sha.to_string(),
                            actual,
                        },
                        Err(_) => GateVerdict::Missing,
                    }
                }
            }
        };
        files.push(GateFileReport {
            file,
            kind,
            verdict,
        });
    };

    for (name, pin) in &build.plugins {
        check(
            data_dir.join(name),
            name.clone(),
            "plugin",
            &pin.sha256,
            pin.size,
        );
    }
    for (rel, pin) in &build.loose_files {
        check(
            data_dir.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR)),
            rel.clone(),
            "loose",
            &pin.sha256,
            pin.size,
        );
    }
    // SKSE plugin DLLs: multiple accepted hashes, case-insensitive filename match
    let skse_dir = data_dir.join("SKSE").join("Plugins");
    for (name, pin) in &dlls.dlls {
        let path = find_case_insensitive(&skse_dir, name);
        let verdict = match path {
            None => GateVerdict::Missing,
            Some(p) => {
                let meta = std::fs::metadata(&p)?;
                let size_ok = pin.size.map(|s| s == meta.len()).unwrap_or(true);
                if !size_ok {
                    GateVerdict::SizeMismatch {
                        expected: pin.size.unwrap_or(0),
                        actual: meta.len(),
                    }
                } else {
                    let actual = sha256_file(&p).map_err(std::io::Error::other)?;
                    if pin.sha256.iter().any(|h| h == &actual) {
                        GateVerdict::Ok
                    } else {
                        GateVerdict::HashMismatch {
                            expected: pin.sha256.join(" | "),
                            actual,
                        }
                    }
                }
            }
        };
        files.push(GateFileReport {
            file: name.clone(),
            kind: "dll",
            verdict,
        });
    }

    files.sort_by(|a, b| {
        let af = !matches!(a.verdict, GateVerdict::Ok);
        let bf = !matches!(b.verdict, GateVerdict::Ok);
        bf.cmp(&af)
            .then_with(|| a.kind.cmp(b.kind))
            .then_with(|| a.file.cmp(&b.file))
    });

    let ok = files
        .iter()
        .filter(|f| matches!(f.verdict, GateVerdict::Ok))
        .count();
    let failed = files.len() - ok;
    Ok(BuildGateReport {
        data_dir: data_dir.to_path_buf(),
        files,
        ok,
        failed,
    })
}

/// Find a direct child of `dir` matching `name` case-insensitively
/// (Wine/NTFS semantics on a case-sensitive Linux fs).
fn find_case_insensitive(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        if entry
            .file_name()
            .to_str()
            .map(|n| n.to_ascii_lowercase() == want)
            .unwrap_or(false)
            && entry.path().is_file()
        {
            return Some(entry.path());
        }
    }
    None
}
