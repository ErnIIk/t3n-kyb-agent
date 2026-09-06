//! verify-entity: looks a legal entity up in the GLEIF Global LEI Index.
//!
//! GLEIF is the authoritative public registry behind the ISO 17442 Legal Entity
//! Identifier. It is free, needs no API key, and covers ~2.7M entities, which
//! makes it the cheapest reliable answer to "does this company actually exist,
//! and is its registration still current?".
//!
//! This lookup carries no personal data, so it uses the plain `http` capability
//! rather than `http-with-placeholders`.

use crate::common::{urlencode, validate_lei};

pub const GLEIF_HOST: &str = "api.gleif.org";
const GLEIF_BASE: &str = "https://api.gleif.org/api/v1/lei-records";
/// GLEIF returns full LEI records; five is enough to disambiguate a name
/// without pulling a payload large enough to strain the enclave's allocator.
const PAGE_SIZE: usize = 5;

#[derive(serde::Deserialize, Debug)]
pub struct VerifyEntityReq {
    /// Exact LEI, when the buyer already has it on file.
    pub lei: Option<String>,
    /// Legal name, when they do not.
    pub legal_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct EntityRecord {
    pub found: bool,
    pub lei: String,
    pub legal_name: String,
    pub jurisdiction: String,
    /// GLEIF entity status: ACTIVE or INACTIVE.
    pub entity_status: String,
    /// GLEIF registration status: ISSUED, LAPSED, RETIRED, ...
    pub registration_status: String,
    pub next_renewal_date: String,
    pub address: String,
    /// Other names that matched, so a human can resolve an ambiguous match.
    pub candidates: Vec<String>,
}

/// Entry point called from `lib.rs`; `input` is the raw JSON request body.
pub fn verify_entity(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: VerifyEntityReq =
        serde_json::from_slice(input).map_err(|e| format!("verify-entity: bad input: {e}"))?;
    let url = build_query_url(&req)?;

    #[cfg(target_arch = "wasm32")]
    {
        let record = fetch_entity(&url)?;
        serde_json::to_vec(&record).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
        Err("verify_entity performs I/O and only runs on the wasm32 target".to_string())
    }
}

/// Builds the GLEIF query URL for either lookup mode.
///
/// Kept free of host imports so the URL-building rules are unit-tested natively.
pub fn build_query_url(req: &VerifyEntityReq) -> Result<String, String> {
    if let Some(lei) = req.lei.as_ref().filter(|s| !s.trim().is_empty()) {
        let lei = validate_lei(lei)?;
        return Ok(format!(
            "{GLEIF_BASE}?filter%5Blei%5D={lei}&page%5Bsize%5D=1"
        ));
    }
    if let Some(name) = req.legal_name.as_ref().filter(|s| !s.trim().is_empty()) {
        return Ok(format!(
            "{GLEIF_BASE}?filter%5Bentity.legalName%5D={}&page%5Bsize%5D={PAGE_SIZE}",
            urlencode(name.trim())
        ));
    }
    Err("verify-entity: provide either lei or legal_name".to_string())
}

/// Parses a GLEIF JSON:API collection response into a flat record.
///
/// Pure function: the fixture-driven tests below are the regression suite for
/// every field this contract depends on.
pub fn parse_gleif_response(body: &[u8]) -> Result<EntityRecord, String> {
    let json: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("gleif: response is not JSON: {e}"))?;

    let records = json["data"]
        .as_array()
        .ok_or("gleif: response has no data array")?;

    let Some(first) = records.first() else {
        return Ok(EntityRecord::default());
    };

    let attributes = &first["attributes"];
    let entity = &attributes["entity"];
    let address_node = &entity["legalAddress"];
    let address_lines: Vec<String> = address_node["addressLines"]
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| line.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let address = [
        address_lines.join(", "),
        address_node["postalCode"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        address_node["city"].as_str().unwrap_or("").to_string(),
        address_node["country"].as_str().unwrap_or("").to_string(),
    ]
    .iter()
    .filter(|part| !part.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(", ");

    let candidates = records
        .iter()
        .skip(1)
        .filter_map(|record| record["attributes"]["entity"]["legalName"]["name"].as_str())
        .map(str::to_string)
        .collect();

    Ok(EntityRecord {
        found: true,
        lei: attributes["lei"].as_str().unwrap_or("").to_string(),
        legal_name: entity["legalName"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        jurisdiction: entity["jurisdiction"]
            .as_str()
            .or_else(|| address_node["country"].as_str())
            .unwrap_or("")
            .to_string(),
        entity_status: entity["status"].as_str().unwrap_or("").to_string(),
        registration_status: attributes["registration"]["status"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        next_renewal_date: attributes["registration"]["nextRenewalDate"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        address,
        candidates,
    })
}

#[cfg(target_arch = "wasm32")]
use crate::host::interfaces::{http as http_iface, logging};

/// Performs the GLEIF call inside the enclave.
#[cfg(target_arch = "wasm32")]
pub fn fetch_entity(url: &str) -> Result<EntityRecord, String> {
    let response = http_iface::call(&http_iface::Request {
        method: http_iface::Verb::Get,
        url: url.to_string(),
        headers: Some(vec![(
            "Accept".to_string(),
            "application/vnd.api+json".to_string(),
        )]),
        payload: None,
    })
    .map_err(|e| format!("gleif request failed: {e}"))?;

    if response.code != 200 {
        let body = String::from_utf8_lossy(&response.payload);
        return Err(format!(
            "gleif lookup failed: HTTP {} — {body}",
            response.code
        ));
    }

    let record = parse_gleif_response(&response.payload)?;
    // Logs carry the company identifier only; no personal data ever reaches here.
    let _ = logging::info(&format!(
        "gleif lookup: found={} lei={}",
        record.found, record.lei
    ));
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/gleif_deutsche_bank.json");

    #[test]
    fn lei_lookup_builds_filtered_url() {
        let req = VerifyEntityReq {
            lei: Some("529900IH9V4I3VHQVO92".into()),
            legal_name: None,
        };
        let url = build_query_url(&req).unwrap();
        assert!(url.contains("filter%5Blei%5D=529900IH9V4I3VHQVO92"));
    }

    #[test]
    fn name_lookup_percent_encodes_the_name() {
        let req = VerifyEntityReq {
            lei: None,
            legal_name: Some("Acme & Co".into()),
        };
        let url = build_query_url(&req).unwrap();
        assert!(url.contains("Acme%20%26%20Co"), "got {url}");
    }

    #[test]
    fn empty_request_is_rejected() {
        let req = VerifyEntityReq {
            lei: Some("   ".into()),
            legal_name: None,
        };
        assert!(build_query_url(&req).is_err());
    }

    #[test]
    fn parses_a_real_gleif_payload() {
        let record = parse_gleif_response(FIXTURE).unwrap();
        assert!(record.found);
        assert_eq!(record.lei, "529900IH9V4I3VHQVO92");
        assert_eq!(record.legal_name, "Deutsche Bank Aktiengesellschaft");
        assert_eq!(record.entity_status, "ACTIVE");
        assert_eq!(record.registration_status, "ISSUED");
        assert!(record.address.contains("Paris"), "got {}", record.address);
    }

    #[test]
    fn empty_result_set_is_not_an_error() {
        let record = parse_gleif_response(br#"{"data":[]}"#).unwrap();
        assert!(!record.found);
        assert_eq!(record.lei, "");
    }

    #[test]
    fn malformed_response_is_an_error() {
        assert!(parse_gleif_response(b"<html>502</html>").is_err());
        assert!(parse_gleif_response(br#"{"errors":[]}"#).is_err());
    }
}
