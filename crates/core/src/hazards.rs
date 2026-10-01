//! The launcher's Verify checks that a green hash gate cannot see.
//!
//! Probed from the live asar. These reads do not move files.
//! - `KNOWN_BAD_FILES` — BeardMaskFix crashes on load-in. Present in Data is a failure.
//! - `Data/Platform/Plugins/*.js` — only `skymp5-client.js` and `rp-portrait.js` are allowed.
//! - Parked Anniversary Edition folders under Data (`_disabledByKzl`, `DisabledBy*`,
//!   or any folder holding a stock master). A stock file in there that is missing
//!   from Data is a failure. A folder that holds nothing missing is not.

use serde::Serialize;
use std::path::{Path, PathBuf};

const KNOWN_BAD: &[&str] = &[
    "SKSE/Plugins/BeardMaskFix.dll",
    "SKSE/Plugins/BeardMaskFix.pdb",
];

const PLATFORM_ALLOWED: &[&str] = &["skymp5-client.js", "rp-portrait.js"];

const STOCK_PLUGINS: &[&str] = &[
    "skyrim.esm",
    "update.esm",
    "dawnguard.esm",
    "hearthfires.esm",
    "dragonborn.esm",
    "ccbgssse001-fish.esm",
    "ccbgssse025-advdsgs.esm",
    "ccbgssse037-curios.esl",
    "ccqdrsse001-survivalmode.esl",
    "_resourcepack.esl",
];

const STOCK_FILES: &[&str] = &[
    "skyrim.esm",
    "update.esm",
    "dawnguard.esm",
    "hearthfires.esm",
    "dragonborn.esm",
    "ccbgssse001-fish.esm",
    "ccbgssse025-advdsgs.esm",
    "ccbgssse037-curios.esl",
    "ccqdrsse001-survivalmode.esl",
    "_resourcepack.esl",
    "ccbgssse001-fish.bsa",
    "ccbgssse025-advdsgs.bsa",
    "ccbgssse037-curios.bsa",
    "ccqdrsse001-survivalmode.bsa",
    "_resourcepack.bsa",
    "skyrim - animations.bsa",
    "skyrim - interface.bsa",
    "skyrim - meshes0.bsa",
    "skyrim - meshes1.bsa",
    "skyrim - misc.bsa",
    "skyrim - shaders.bsa",
    "skyrim - sounds.bsa",
    "skyrim - textures0.bsa",
    "skyrim - textures1.bsa",
    "skyrim - textures2.bsa",
    "skyrim - textures3.bsa",
    "skyrim - textures4.bsa",
    "skyrim - textures5.bsa",
    "skyrim - textures6.bsa",
    "skyrim - textures7.bsa",
    "skyrim - textures8.bsa",
    "skyrim - voices_en0.bsa",
    "marketplacetextures.bsa",
];

#[derive(Debug, Clone, Serialize)]
pub struct HazardIssue {
    pub kind: String,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HazardReport {
    pub known_bad: Vec<String>,
    pub foreign_plugins: Vec<String>,
    pub parked_folders: Vec<String>,
    pub failed: usize,
    pub note: String,
    pub issues: Vec<HazardIssue>,
}

pub fn read_hazards(skyrim_root: &Path) -> HazardReport {
    let data = skyrim_root.join("Data");
    let mut issues = Vec::new();

    let mut known_bad = Vec::new();
    for rel in KNOWN_BAD {
        if let Some(path) = resolve_ci(&data, rel) {
            if path.is_file() {
                known_bad.push((*rel).to_string());
                issues.push(HazardIssue {
                    kind: "known-bad".into(),
                    name: (*rel).to_string(),
                    detail: "BeardMaskFix crashes on load-in. The launcher quarantines it.".into(),
                });
            }
        }
    }

    let mut foreign_plugins = Vec::new();
    if let Some(dir) = resolve_ci(&data, "Platform/Plugins") {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut names: Vec<String> = rd
                .flatten()
                .filter_map(|ent| {
                    let name = ent.file_name().to_string_lossy().to_string();
                    if !name.to_ascii_lowercase().ends_with(".js") {
                        return None;
                    }
                    if PLATFORM_ALLOWED
                        .iter()
                        .any(|ok| ok.eq_ignore_ascii_case(&name))
                    {
                        return None;
                    }
                    Some(name)
                })
                .collect();
            names.sort();
            for name in &names {
                issues.push(HazardIssue {
                    kind: "foreign-platform".into(),
                    name: name.clone(),
                    detail: "Other server's client plugin. Not in the launcher allowlist.".into(),
                });
            }
            foreign_plugins = names;
        }
    }

    let mut parked_folders = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&data) {
        let mut dirs: Vec<PathBuf> = rd
            .flatten()
            .map(|ent| ent.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for folder in dirs {
            let name = folder
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !is_parked_candidate(&folder, &name) {
                continue;
            }
            let restorable = restorable_stock(&data, &folder);
            if restorable.is_empty() {
                continue;
            }
            parked_folders.push(name.clone());
            issues.push(HazardIssue {
                kind: "parked-content".into(),
                name,
                detail: format!(
                    "{} stock file(s) sit in this folder and are missing from Data.",
                    restorable.len()
                ),
            });
        }
    }

    let note = if issues.is_empty() {
        "No known-bad files, no foreign client plugins, no parked Anniversary Edition files."
            .to_string()
    } else {
        format!(
            "{} known-bad, {} foreign client plugin(s), {} parked folder(s).",
            known_bad.len(),
            foreign_plugins.len(),
            parked_folders.len()
        )
    };

    HazardReport {
        failed: issues.len(),
        note,
        known_bad,
        foreign_plugins,
        parked_folders,
        issues,
    }
}

fn is_parked_candidate(folder: &Path, name: &str) -> bool {
    let norm = normalize_folder(name);
    if norm == "disabledbkzl" || norm == "disabledbykzl" || is_disabled_by(&norm) {
        return true;
    }
    holds_stock_plugin(folder)
}

fn is_disabled_by(norm: &str) -> bool {
    let rest = norm.strip_prefix("disabledby").unwrap_or("");
    norm.starts_with("disabledby") && rest.chars().all(|c| c.is_ascii_alphanumeric())
}

fn normalize_folder(name: &str) -> String {
    name.to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn holds_stock_plugin(folder: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(folder) else {
        return false;
    };
    rd.flatten().any(|ent| {
        let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
        ent.path().is_file() && STOCK_PLUGINS.contains(&name.as_str())
    })
}

fn restorable_stock(data: &Path, folder: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk_stock(folder, folder, data, &mut out);
    out.sort();
    out
}

fn walk_stock(root: &Path, dir: &Path, data: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        if path.is_dir() {
            walk_stock(root, &path, data, out);
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if !is_stock_file(&rel) {
            continue;
        }
        if resolve_ci(data, &rel).is_none() {
            out.push(rel);
        }
    }
}

fn is_stock_file(rel: &str) -> bool {
    let rel = rel.trim_start_matches('/').to_ascii_lowercase();
    if STOCK_FILES.contains(&rel.as_str()) {
        return true;
    }
    let name = rel.rsplit('/').next().unwrap_or(&rel);
    if name.starts_with("skyrim - voices_") && name.ends_with(".bsa") {
        let mid = &name["skyrim - voices_".len()..name.len() - 4];
        let chars: Vec<char> = mid.chars().collect();
        if chars.len() >= 2
            && chars[0].is_ascii_alphabetic()
            && chars[1].is_ascii_alphabetic()
            && chars[2..].iter().all(|c| c.is_ascii_digit())
        {
            return rel == name;
        }
    }
    if let Some(file) = rel.strip_prefix("strings/") {
        if !file.contains('/')
            && (file.ends_with(".strings")
                || file.ends_with(".dlstrings")
                || file.ends_with(".ilstrings"))
        {
            return true;
        }
    }
    if let Some(file) = rel.strip_prefix("video/") {
        if !file.contains('/') && file.ends_with(".bik") {
            return true;
        }
    }
    false
}

fn resolve_ci(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut cur = root.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|p| !p.is_empty()) {
        let want = part.to_ascii_lowercase();
        let next = std::fs::read_dir(&cur).ok()?.flatten().find(|ent| {
            ent.file_name().to_string_lossy().to_ascii_lowercase() == want
        })?;
        cur = next.path();
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_game(label: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "daedric-hazards-{label}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("Data/Platform/Plugins")).unwrap();
        p
    }

    #[test]
    fn a_clean_data_dir_is_not_a_failure() {
        let root = temp_game("clean");
        std::fs::write(
            root.join("Data/Platform/Plugins/skymp5-client.js"),
            b"ok",
        )
        .unwrap();
        let report = read_hazards(&root);
        assert_eq!(report.failed, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn beardmaskfix_and_a_foreign_js_fail() {
        let root = temp_game("bad");
        let plug = root.join("Data/SKSE/Plugins");
        std::fs::create_dir_all(&plug).unwrap();
        std::fs::write(plug.join("BeardMaskFix.dll"), b"x").unwrap();
        std::fs::write(root.join("Data/Platform/Plugins/mereth-client.js"), b"x").unwrap();
        let report = read_hazards(&root);
        assert!(report.known_bad.iter().any(|n| n.contains("BeardMaskFix")));
        assert_eq!(report.foreign_plugins, vec!["mereth-client.js".to_string()]);
        assert!(report.failed >= 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn parked_stock_missing_from_data_fails() {
        let root = temp_game("parked");
        let parked = root.join("Data/_disabledByKzl");
        std::fs::create_dir_all(&parked).unwrap();
        std::fs::write(parked.join("Skyrim.esm"), b"master").unwrap();
        let report = read_hazards(&root);
        assert_eq!(report.parked_folders, vec!["_disabledByKzl".to_string()]);
        assert!(report.failed >= 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn parked_file_already_in_data_does_not_fail() {
        let root = temp_game("present");
        let parked = root.join("Data/DisabledByKZL");
        std::fs::create_dir_all(&parked).unwrap();
        std::fs::write(parked.join("Update.esm"), b"copy").unwrap();
        std::fs::write(root.join("Data/Update.esm"), b"live").unwrap();
        let report = read_hazards(&root);
        assert!(report.parked_folders.is_empty());
        assert_eq!(report.failed, 0);
        let _ = std::fs::remove_dir_all(root);
    }
}
