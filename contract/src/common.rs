//! Shared helpers: tenant KV access, input sanitising, and URL encoding.
//!
//! Everything here that does not need a host import compiles on the native
//! target too, so `cargo test` exercises it without a WASM runtime.

/// Percent-encodes a query-string value.
///
/// Company names arrive from the caller and go straight into a GLEIF query
/// string; without encoding, a name containing `&` or `#` would silently
/// truncate or rewrite the query.
pub fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Rejects anything that is not a 2-letter ISO country code.
///
/// The value is interpolated into a VIES path segment, so a permissive check
/// here would let a caller walk into a different endpoint.
pub fn validate_country_code(code: &str) -> Result<String, String> {
    let upper = code.trim().to_ascii_uppercase();
    if upper.len() == 2 && upper.bytes().all(|b| b.is_ascii_uppercase()) {
        Ok(upper)
    } else {
        Err(format!(
            "invalid country_code {code:?}: expected two letters, e.g. DE"
        ))
    }
}

/// Rejects anything that is not an alphanumeric VAT body (2-14 chars).
///
/// Same reasoning as `validate_country_code`: this lands in a URL path.
pub fn validate_vat_number(vat: &str) -> Result<String, String> {
    let cleaned: String = vat
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '.')
        .collect::<String>()
        .to_ascii_uppercase();
    if (2..=14).contains(&cleaned.len()) && cleaned.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(cleaned)
    } else {
        Err(format!(
            "invalid vat_number {vat:?}: expected 2-14 alphanumeric characters"
        ))
    }
}

/// A LEI is exactly 20 upper-case alphanumeric characters (ISO 17442).
pub fn validate_lei(lei: &str) -> Result<String, String> {
    let upper = lei.trim().to_ascii_uppercase();
    if upper.len() == 20 && upper.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(upper)
    } else {
        Err(format!(
            "invalid lei {lei:?}: expected 20 alphanumeric characters (ISO 17442)"
        ))
    }
}

/// Case- and punctuation-insensitive comparison of two company names.
///
/// "ACME GmbH" and "Acme G.m.b.H." are the same supplier to a human reviewer;
/// the KYB score should not flag that as a name mismatch.
pub fn names_match(a: &str, b: &str) -> bool {
    let normalise = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    let (a, b) = (normalise(a), normalise(b));
    !a.is_empty() && !b.is_empty() && (a == b || a.contains(&b) || b.contains(&a))
}

#[cfg(target_arch = "wasm32")]
use crate::host::{interfaces::kv_store, tenant::tenant_context};

/// Reads one key from a tenant KV map, addressed as `z:<tid>:<map>`.
///
/// The tenant id comes from the host at runtime rather than being baked in, so
/// the same WASM artifact works for any tenant that registers it.
#[cfg(target_arch = "wasm32")]
pub fn kv_get(map: &str, key: &str) -> Result<Option<String>, String> {
    let tid = tenant_context::tenant_did();
    let map_name = format!("z:{}:{}", hex::encode(&tid), map);
    match kv_store::get(&map_name, key.as_bytes()) {
        Ok(Some(bytes)) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|e| format!("kv value for {key} is not utf-8: {e}")),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("kv read {map_name}/{key} failed: {e}")),
    }
}

/// Reads a `config` entry, falling back to a compiled-in default.
///
/// Endpoints live in KV rather than in the binary so that pointing the agent at
/// a different procurement system is a one-line tenant update, not a rebuild
/// and re-registration of the contract.
#[cfg(target_arch = "wasm32")]
pub fn config_or(key: &str, default: &str) -> String {
    match kv_get("config", key) {
        Ok(Some(value)) if !value.trim().is_empty() => value,
        _ => default.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_escapes_query_breaking_characters() {
        assert_eq!(urlencode("Acme & Co"), "Acme%20%26%20Co");
        assert_eq!(urlencode("Bosch-Werke_1.0~x"), "Bosch-Werke_1.0~x");
    }

    #[test]
    fn country_code_accepts_two_letters_and_uppercases() {
        assert_eq!(validate_country_code("de").unwrap(), "DE");
        assert_eq!(validate_country_code(" fr ").unwrap(), "FR");
    }

    #[test]
    fn country_code_rejects_path_traversal() {
        assert!(validate_country_code("../admin").is_err());
        assert!(validate_country_code("DEU").is_err());
        assert!(validate_country_code("").is_err());
    }

    #[test]
    fn vat_number_strips_formatting_and_rejects_junk() {
        assert_eq!(validate_vat_number("811 907 980").unwrap(), "811907980");
        assert_eq!(validate_vat_number("IE-6388047V").unwrap(), "IE6388047V");
        assert!(validate_vat_number("../../etc/passwd").is_err());
        assert!(validate_vat_number("1").is_err());
    }

    #[test]
    fn lei_must_be_twenty_alphanumerics() {
        assert!(validate_lei("529900IH9V4I3VHQVO92").is_ok());
        assert!(validate_lei("TOO-SHORT").is_err());
    }

    #[test]
    fn names_match_ignores_case_and_punctuation() {
        assert!(names_match("Acme GmbH", "ACME G.m.b.H."));
        assert!(names_match(
            "Deutsche Bank Aktiengesellschaft",
            "Deutsche Bank"
        ));
        assert!(!names_match("Acme GmbH", "Globex Ltd"));
        assert!(!names_match("", "Acme"));
    }
}
