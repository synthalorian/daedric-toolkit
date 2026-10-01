//! Quarantine, Plugins.txt, and the install ledger against a live install.
//! Does not hash the Data dir.
//!
//! usage: field [asar_path] [skyrim_root]
//! or set DAEDRIC_ASAR and DAEDRIC_SKYRIM.

use daedric_core::{
    expected_load_order, plugin_set, read_install_ledger, read_load_order, read_quarantine, Asar,
    BuildManifest, CollectionLock,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn path_arg(nth: usize, env_key: &str) -> PathBuf {
    std::env::args()
        .nth(nth)
        .or_else(|| std::env::var(env_key).ok())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("usage: field [asar_path] [skyrim_root]  (or DAEDRIC_ASAR / DAEDRIC_SKYRIM)");
            std::process::exit(2);
        })
}

fn main() {
    let asar = path_arg(1, "DAEDRIC_ASAR");
    let root = path_arg(2, "DAEDRIC_SKYRIM");
    let lock = CollectionLock::from_asar(&asar).expect("collection-lock");
    let build = BuildManifest::from_asar(&asar).expect("build-manifest");
    let plugins = plugin_set(&lock, &build);
    let order_plugins = expected_load_order(&asar, &plugins);

    let mut names = BTreeMap::new();
    if let Ok(archive) = Asar::open(&asar) {
        if let Ok(value) = archive.read_json::<serde_json::Value>("data/modlist.json") {
            names = daedric_core::names_from_modlist(&value);
        }
    }
    for pin in &lock.mods {
        if let Some(id) = pin.mod_id {
            if !pin.name.trim().is_empty() {
                names.insert(id.to_string(), pin.name.clone());
            }
        }
    }

    let quarantine = read_quarantine(&root, &plugins);
    println!(
        "quarantine found={} foreign={} contract_hits={} — {}",
        quarantine.found, quarantine.foreign, quarantine.contract_hits, quarantine.note
    );
    for batch in &quarantine.batches {
        for file in &batch.files {
            println!(
                "  {} {}{}",
                batch.at,
                file.name,
                if file.in_contract { " CONTRACT" } else { "" }
            );
        }
    }

    let order = read_load_order(&root, &asar, &order_plugins);
    println!(
        "load-order found={} path={} enabled={} contract_ok={} failed={} — {}",
        order.found,
        order
            .path
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".into()),
        order.enabled,
        order.contract_ok,
        order.failed,
        order.note
    );
    for issue in &order.issues {
        println!("  {} {}", issue.kind, issue.name);
    }

    let ledger = read_install_ledger(&root, &names);
    println!(
        "ledger found={} recorded={} named={} — {}",
        ledger.found, ledger.recorded, ledger.named, ledger.note
    );
    for m in &ledger.mods {
        println!(
            "  {} {} — {} files, {} plugins",
            m.mod_id,
            if m.name.is_empty() { "—" } else { &m.name },
            m.file_count,
            m.plugin_count
        );
    }
}
