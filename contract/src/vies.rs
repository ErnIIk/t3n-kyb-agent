//! check-vat: validates an EU VAT number against the European Commission's
//! VIES service.
//!
//! VIES is the only authoritative source for "is this VAT number registered
//! right now", and an invalid VAT number is the most common reason a supplier
//! invoice is later rejected by a tax authority.
//!
//! Public endpoint, no API key, no PII in the request — plain `http`.

use crate::common::{validate_country_code, validate_vat_number};

pub const VIES_HOST: &str = "ec.europa.eu";
const VIES_BASE: &str = "https://ec.europa.eu/taxation_customs/vies/rest-api";

#[derive(serde::Deserialize, Debug)]
pub struct CheckVatReq {
    pub country_code: String,
    pub vat_number: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct VatRecord {
    pub valid: bool,
    pub country_code: String,
    pub vat_number: String,
    /// VIES returns "---" when the member state withholds the trader name.
    pub name: String,
    pub address: String,
    pub request_date: String,
    /// VIES `userError`: VALID, INVALID_INPUT, MS_UNAVAILABLE, ...
    pub service_status: String,
}

pub fn check_vat(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: CheckVatReq =
        serde_json::from_slice(input).map_err(|e| format!("check-vat: bad input: {e}"))?;
    let url = build_vat_url(&req.country_code, &req.vat_number)?;

    #[cfg(target_arch = "wasm32")]
    {
        let record = fetch_vat(&url, &req)?;
        serde_json::to_vec(&record).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
        Err("check_vat performs I/O and only runs on the wasm32 target".to_string())
    }
}

/// Builds the VIES path, validating both segments first.
pub fn build_vat_url(country_code: &str, vat_number: &str) -> Result<String, String> {
    let country = validate_country_code(country_code)?;
    let vat = validate_vat_number(vat_number)?;
    Ok(format!("{VIES_BASE}/ms/{country}/vat/{vat}"))
}

/// Parses the VIES REST response.
///
/// Note the field name: the GET endpoint returns `isValid`, while the POST
/// endpoint returns `valid`. Both spellings are accepted so that switching
/// transport later cannot silently turn every supplier into "VAT invalid".
pub fn parse_vies_response(body: &[u8], country_code: &str) -> Result<VatRecord, String> {
    let json: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("vies: response is not JSON: {e}"))?;

    let valid = json["isValid"]
        .as_bool()
        .or_else(|| json["valid"].as_bool())
        .ok_or("vies: response has neither isValid nor valid")?;

    let clean = |value: &serde_json::Value| -> String {
        match value.as_str() {
            // VIES uses "---" as its "withheld by member state" marker.
            Some("---") | None => String::new(),
            Some(other) => other.to_string(),
        }
    };

    Ok(VatRecord {
        valid,
        country_code: json["countryCode"]
            .as_str()
            .unwrap_or(country_code)
            .to_string(),
        vat_number: json["vatNumber"].as_str().unwrap_or("").to_string(),
        name: clean(&json["name"]),
        address: clean(&json["address"]),
        request_date: json["requestDate"].as_str().unwrap_or("").to_string(),
        service_status: json["userError"].as_str().unwrap_or("").to_string(),
    })
}

#[cfg(target_arch = "wasm32")]
use crate::host::interfaces::{http as http_iface, logging};

#[cfg(target_arch = "wasm32")]
pub fn fetch_vat(url: &str, req: &CheckVatReq) -> Result<VatRecord, String> {
    let response = http_iface::call(&http_iface::Request {
        method: http_iface::Verb::Get,
        url: url.to_string(),
        headers: Some(vec![("Accept".to_string(), "application/json".to_string())]),
        payload: None,
    })
    .map_err(|e| format!("vies request failed: {e}"))?;

    if response.code != 200 {
        let body = String::from_utf8_lossy(&response.payload);
        return Err(format!(
            "vies lookup failed: HTTP {} — {body}",
            response.code
        ));
    }

    let record = parse_vies_response(&response.payload, &req.country_code)?;
    let _ = logging::info(&format!(
        "vies check: {}{} valid={}",
        record.country_code, record.vat_number, record.valid
    ));
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/vies_valid.json");

    #[test]
    fn builds_a_validated_path() {
        assert_eq!(
            build_vat_url("de", "811 907 980").unwrap(),
            "https://ec.europa.eu/taxation_customs/vies/rest-api/ms/DE/vat/811907980"
        );
    }

    #[test]
    fn rejects_traversal_in_either_segment() {
        assert!(build_vat_url("../x", "811907980").is_err());
        assert!(build_vat_url("DE", "../../admin").is_err());
    }

    #[test]
    fn parses_a_real_vies_payload() {
        let record = parse_vies_response(FIXTURE, "DE").unwrap();
        assert!(record.valid);
        assert_eq!(record.vat_number, "811907980");
        assert_eq!(record.service_status, "VALID");
        // "---" is a withheld marker, not a company name.
        assert_eq!(record.name, "");
    }

    #[test]
    fn accepts_the_post_endpoint_spelling_too() {
        let record = parse_vies_response(br#"{"valid":true,"vatNumber":"1"}"#, "DE").unwrap();
        assert!(record.valid);
    }

    #[test]
    fn missing_validity_flag_is_an_error() {
        assert!(parse_vies_response(br#"{"vatNumber":"1"}"#, "DE").is_err());
    }
}
