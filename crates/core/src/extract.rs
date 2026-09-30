//! Tolerant extraction of JSON data embedded in the launcher's esbuild bundle.
//!
//! The launcher's data modules are CommonJS modules inside `dist/main.js`:
//! `module2.exports = { ...JS object literal... };` with UNQUOTED keys.
//! These helpers locate a module by its `data/<name>.json` marker comment,
//! slice out the balanced object, and quote bare keys so serde_json can parse.

/// Find the `module2.exports = { ... }` object for a given module marker
/// (e.g. `"data/collection-lock.json"`) and return it with all bare keys quoted,
/// ready for `serde_json::from_str`.
pub fn extract_module_json(text: &str, marker: &str) -> Option<String> {
    let m = text.find(marker)?;
    let exports = text[m..].find("module2.exports").map(|e| m + e)?;
    let brace = text[exports..].find('{').map(|b| exports + b)?;
    let slice = balanced_object(text.as_bytes(), brace)?;
    Some(quote_bare_keys(slice))
}

/// Given a byte offset at a `{`, return the balanced `{...}` slice as &str.
pub fn balanced_object(bytes: &[u8], start: usize) -> Option<&str> {
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    end.and_then(|e| std::str::from_utf8(&bytes[start..e]).ok())
}

/// Quote bare JS-object-literal keys: `{ sha256: "..." }` -> `{ "sha256": "..." }`.
/// Only touches identifier keys after `{` or `,` outside strings.
pub fn quote_bare_keys(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + input.len() / 8);
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            out.push(b as char);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if (b == b'{' || b == b',') && !in_string {
            out.push(b as char);
            i += 1;
            // skip whitespace
            while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                out.push(bytes[i] as char);
                i += 1;
            }
            // bare identifier key followed by ':'?
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            if i > start {
                let mut j = i;
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b':' {
                    out.push('"');
                    out.push_str(&input[start..i]);
                    out.push('"');
                    continue; // whitespace before ':' emitted next loop
                } else {
                    out.push_str(&input[start..i]);
                    continue;
                }
            }
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_module_by_marker() {
        let js = r#"// data/thing.json
var require_thing = __commonJS({
  "data/thing.json"(exports2, module2) {
    module2.exports = {
      schema: "test/1",
      items: { "a.esp": { sha256: "abc", size: 1 } }
    };
  }
});"#;
        let q = extract_module_json(js, "data/thing.json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&q).unwrap();
        assert_eq!(v["schema"], "test/1");
        assert_eq!(v["items"]["a.esp"]["size"], 1);
    }
}
