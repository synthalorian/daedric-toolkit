//! daedric-core — the engine under the Daedric Toolkit.
//!
//! Born from field forensics (see daedric-online-linux/docs/the-mod-verification-gate.md):
//! the Daedric Online launcher bundles its mod-verification contract inside its Electron
//! `app.asar` — required mod list, canonical archive checksums, and a per-file sha256 map.
//! This crate decodes that contract and checks a real install against it.

pub mod hash;
pub mod manifest;
pub mod scan;

pub use manifest::Manifest;
pub use scan::{scan_install, FileVerdict, ScanReport};
