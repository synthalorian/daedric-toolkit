//! daedric-core — the engine under the Daedric Toolkit.
//!
//! Born from field forensics (see daedric-online-linux/docs/the-mod-verification-gate.md):
//! the Daedric Online launcher bundles its mod-verification contract inside its Electron
//! `app.asar` — required mod list, canonical archive checksums, and a per-file sha256 map.
//! This crate decodes that contract and checks a real install against it.

pub mod asar;
pub mod collection;
mod de;
pub mod extract;
pub mod hash;
pub mod manifest;
pub mod overlay;
pub mod scan;
pub mod verify;

pub use asar::Asar;
pub use collection::{BuildManifest, CollectionLock, DllAllowlist};
pub use manifest::Manifest;
pub use overlay::{check_overlay, read_installed_overlay, OverlayReport, OverlayVerdict};
pub use scan::{scan_install, FileVerdict, ScanReport};
pub use verify::{scan_build_gate, verify_downloads, BuildGateReport, DownloadsReport};
