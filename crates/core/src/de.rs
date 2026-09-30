//! Lenient deserializers — the launcher's own tooling writes inconsistent
//! JSON: u64s appear as ints, floats (`53000.0`), or decimal strings
//! (asar directory offsets). Accept all three.

use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(untagged)]
enum NumOrStr {
    Int(u64),
    Float(f64),
    Str(String),
}

pub fn u64_lenient<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    match NumOrStr::deserialize(d)? {
        NumOrStr::Int(n) => Ok(n),
        NumOrStr::Float(f) => Ok(f as u64),
        NumOrStr::Str(s) => s.trim().parse().map_err(serde::de::Error::custom),
    }
}

pub fn opt_u64_lenient<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    match Option::<NumOrStr>::deserialize(d)? {
        None => Ok(None),
        Some(NumOrStr::Int(n)) => Ok(Some(n)),
        Some(NumOrStr::Float(f)) => Ok(Some(f as u64)),
        Some(NumOrStr::Str(s)) => {
            let s = s.trim();
            if s.is_empty() || s == "null" {
                Ok(None)
            } else {
                s.parse().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}
