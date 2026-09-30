//! Installed-overlay drift detection.
//!
//! The launcher records which overlay build is installed in
//! `<game>/DaedricData/overlay-version.json` (`{"version": "4.99.597"}`),
//! with legacy fallback `<game>/.daedric-overlay.json`. The set of overlay
//! versions the current contract accepts ships in the skse-dll-allowlist
//! module's `overlayVersions` array.
//!
//! An installed version outside the accepted set means DRIFT — the launcher
//! will push an update at next Play — not file corruption. Gate failures on
//! a drifting install should be reported as "behind the contract".

use crate::collection::DllAllowlist;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OverlayVerdict {
    /// Installed version is in the contract's accepted set.
    Current,
    /// Installed version is NOT in the accepted set — the launcher will
    /// push an update at next Play. Gate failures are drift, not corruption.
    Drift,
    /// No overlay marker found (fresh install, or overlay never applied).
    NotInstalled,
    /// The contract carries no accepted set — nothing to compare against.
    UnknownContract,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverlayReport {
    pub verdict: OverlayVerdict,
    /// Version string from the on-disk marker, if any.
    pub installed: Option<String>,
    /// Accepted versions from the contract (allowlist `overlayVersions`).
    pub accepted: Vec<String>,
    /// True when the version came from the legacy `.daedric-overlay.json`.
    pub legacy_marker: bool,
    /// Installed sorts strictly below the highest accepted version.
    pub behind_latest: bool,
}

#[derive(serde::Deserialize)]
struct VersionMarker {
    version: Option<String>,
}

fn read_marker(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<VersionMarker>(&text)
        .ok()?
        .version
        .filter(|v| !v.trim().is_empty())
}

/// Read the installed overlay version: current marker first, legacy fallback.
/// Returns `(version, used_legacy_marker)`.
pub fn read_installed_overlay(skyrim_root: &Path) -> Option<(String, bool)> {
    let current = skyrim_root.join("DaedricData").join("overlay-version.json");
    if let Some(v) = read_marker(&current) {
        return Some((v, false));
    }
    let legacy = skyrim_root.join(".daedric-overlay.json");
    read_marker(&legacy).map(|v| (v, true))
}

/// Loose dotted-numeric compare ("4.99.597" -> [4, 99, 597]). Non-numeric
/// components coerce to 0 — good enough for the launcher's version scheme.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Compare the installed overlay marker against the contract's accepted set.
pub fn check_overlay(skyrim_root: &Path, allowlist: &DllAllowlist) -> OverlayReport {
    let accepted = allowlist.overlay_versions.clone();
    let installed = read_installed_overlay(skyrim_root);

    let verdict = if accepted.is_empty() {
        OverlayVerdict::UnknownContract
    } else if installed.is_none() {
        OverlayVerdict::NotInstalled
    } else if accepted.contains(&installed.as_ref().unwrap().0) {
        OverlayVerdict::Current
    } else {
        OverlayVerdict::Drift
    };

    let behind_latest = match (&installed, accepted.iter().max_by_key(|v| version_key(v))) {
        (Some((inst, _)), Some(latest)) => version_key(inst) < version_key(latest),
        _ => false,
    };

    let (installed_version, legacy_marker) = match installed {
        Some((v, l)) => (Some(v), l),
        None => (None, false),
    };

    OverlayReport {
        verdict,
        installed: installed_version,
        accepted,
        legacy_marker,
        behind_latest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::DllAllowlist;
    use std::collections::BTreeMap;

    fn allowlist(versions: &[&str]) -> DllAllowlist {
        DllAllowlist {
            generated_at: None,
            overlay_versions: versions.iter().map(|s| s.to_string()).collect(),
            dlls: BTreeMap::new(),
        }
    }

    #[test]
    fn drift_when_installed_not_in_accepted_set() {
        let dir = std::env::temp_dir().join(format!("daedric-overlay-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("DaedricData")).unwrap();
        std::fs::write(
            dir.join("DaedricData").join("overlay-version.json"),
            r#"{"version":"4.99.597"}"#,
        )
        .unwrap();
        let report = check_overlay(&dir, &allowlist(&["4.99.596", "4.99.608"]));
        assert_eq!(report.verdict, OverlayVerdict::Drift);
        assert_eq!(report.installed.as_deref(), Some("4.99.597"));
        assert!(report.behind_latest);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn current_when_in_accepted_set() {
        let dir =
            std::env::temp_dir().join(format!("daedric-overlay-test2-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("DaedricData")).unwrap();
        std::fs::write(
            dir.join("DaedricData").join("overlay-version.json"),
            r#"{"version":"4.99.608"}"#,
        )
        .unwrap();
        let report = check_overlay(&dir, &allowlist(&["4.99.596", "4.99.608"]));
        assert_eq!(report.verdict, OverlayVerdict::Current);
        assert!(!report.behind_latest);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_marker_fallback() {
        let dir =
            std::env::temp_dir().join(format!("daedric-overlay-test3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".daedric-overlay.json"),
            r#"{"version":"4.99.596"}"#,
        )
        .unwrap();
        let report = check_overlay(&dir, &allowlist(&["4.99.596", "4.99.608"]));
        assert_eq!(report.verdict, OverlayVerdict::Current);
        assert!(report.legacy_marker);
        assert!(report.behind_latest);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn not_installed_and_unknown_contract() {
        let dir =
            std::env::temp_dir().join(format!("daedric-overlay-test4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            check_overlay(&dir, &allowlist(&["4.99.608"])).verdict,
            OverlayVerdict::NotInstalled
        );
        assert_eq!(
            check_overlay(&dir, &allowlist(&[])).verdict,
            OverlayVerdict::UnknownContract
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
