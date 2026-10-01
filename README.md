# Daedric Toolkit

Install doctor for [Daedric Online](https://daedriconline.com). It reads the launcher's `app.asar` contract and checks a Skyrim Special Edition install against it: plugin hashes, loose-file pins, SKSE DLL allowlist, archive checksums, overlay-version drift, quarantined plugins, the Plugins.txt load order, the launcher's install ledger, the SkyrimSE.exe build, the release book, and the play-elsewhere toggle.

A failing gate is often version drift, not a corrupt file. If the installed overlay is older than the contract, the doctor says so instead of calling the files broken.

## Build

Requires Rust and the Tauri 2 CLI.

```bash
cargo test -p daedric-core
npx --yes @tauri-apps/cli@2 build
```

The frontend is static HTML/CSS/JS in `frontend/dist`. No bundler. Path fields are empty until you fill them; the app remembers them locally after that.

## Field check

```bash
export DAEDRIC_ASAR="/path/to/DaedricOnline/resources/app.asar"
export DAEDRIC_SKYRIM="/path/to/Skyrim Special Edition"
cargo run -p daedric-core --example decode
cargo run -p daedric-core --example doctor
```

Paths are resolved case-insensitively, one component at a time. That matches Wine on a case-sensitive Linux filesystem.

## License

Apache-2.0. See [LICENSE](LICENSE).
