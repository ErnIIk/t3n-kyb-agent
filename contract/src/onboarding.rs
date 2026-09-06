//! submit-onboarding: files the supplier record with the buyer's procurement
//! system, without the contract ever holding the submitter's personal data.
//!
//! The body is built with `{{profile.<field>}}` markers. The host resolves them
//! inside the enclave, at dispatch time, from the calling user's verified
//! profile — and only if that user has authorised this agent for this function
//! and this host. From this module's point of view the name and email are never
//! more than literal template strings.

/// Where the record goes when the tenant has not configured an endpoint.
/// httpbin echoes the request back, which makes the placeholder substitution
/// visible end to end during a demo or a smoke test.
pub const DEFAULT_ONBOARDING_URL: &str = "https://httpbin.org/post";
pub const DEFAULT_ONBOARDING_HOST: &str = "httpbin.org";
/// KV key in the tenant `config` map that overrides the endpoint.
pub const ONBOARDING_URL_KEY: &str = "onboarding_url";

#[derive(serde::Deserialize, Debug)]
pub struct OnboardingReq {
    pub legal_name: String,
    pub lei: Option<String>,
    pub vat_number: Option<String>,
    /// Verdict carried over from `run-kyb-check`.
    pub verdict: String,
    pub risk_score: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct OnboardingResp {
    pub submitted: bool,
    pub status_code: u16,
    /// Reference id echoed by the procurement system, when it returns one.
    pub reference: String,
    pub endpoint_host: String,
}

pub fn submit_onboarding(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: OnboardingReq =
        serde_json::from_slice(input).map_err(|e| format!("submit-onboarding: bad input: {e}"))?;
    if req.legal_name.trim().is_empty() {
        return Err("submit-onboarding: legal_name is required".to_string());
    }

    #[cfg(target_arch = "wasm32")]
    {
        let response = submit_wasm(req)?;
        serde_json::to_vec(&response).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("submit_onboarding performs I/O and only runs on the wasm32 target".to_string())
    }
}

/// Builds the onboarding payload.
///
/// The three `{{profile.*}}` markers are the whole point of this function, so
/// the test below asserts that no real personal data can be smuggled in beside
/// them: everything else in the body is company data.
pub fn build_payload(req: &OnboardingReq) -> serde_json::Value {
    serde_json::json!({
        "source": "t3n-kyb-agent",
        "supplier": {
            "legal_name": req.legal_name,
            "lei": req.lei.clone().unwrap_or_default(),
            "vat_number": req.vat_number.clone().unwrap_or_default(),
            "kyb_verdict": req.verdict,
            "kyb_risk_score": req.risk_score,
        },
        // Resolved by the host inside the enclave; never present in WASM memory.
        "submitted_by": {
            "first_name": "{{profile.first_name}}",
            "last_name": "{{profile.last_name}}",
            "email": "{{profile.verified_contacts.email.value}}",
        },
    })
}

/// Extracts the hostname from a URL, for error messages and egress diagnostics.
///
/// An `egress_denied` failure is the single most common first-run error, and it
/// is far easier to fix when the message names the host that must be added to
/// the grant.
pub fn host_of(url: &str) -> String {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Pulls a reference id out of whatever the procurement system returned.
///
/// Different systems name it differently, and httpbin returns neither, so an
/// absent reference is normal rather than an error.
pub fn extract_reference(body: &[u8]) -> String {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) else {
        return String::new();
    };
    for key in ["reference", "id", "request_id", "ticket"] {
        if let Some(value) = json[key].as_str() {
            return value.to_string();
        }
    }
    String::new()
}

#[cfg(target_arch = "wasm32")]
use crate::{
    common::config_or,
    host::interfaces::{http_with_placeholders as hwp, logging},
};

#[cfg(target_arch = "wasm32")]
fn submit_wasm(req: OnboardingReq) -> Result<OnboardingResp, String> {
    let url = config_or(ONBOARDING_URL_KEY, DEFAULT_ONBOARDING_URL);
    let host = host_of(&url);
    let payload = build_payload(&req);

    let response = hwp::call(&hwp::Request {
        method: hwp::Verb::Post,
        url: url.clone(),
        headers: Some(vec![("Accept".to_string(), "application/json".to_string())]),
        payload: Some(serde_json::to_vec(&payload).map_err(|e| e.to_string())?),
    })
    .map_err(|e| format_http_error(e, &host))?;

    let submitted = (200..300).contains(&response.code);
    if !submitted {
        // Deliberately no response body here, unlike the GLEIF and VIES paths.
        //
        // This is the one request whose body the host filled with the user's
        // real name and email, and a rejecting endpoint commonly echoes the
        // request back — httpbin, the default endpoint, echoes it always. Any
        // of that body in this error would carry plaintext PII out of the
        // enclave, into the agent's stderr, and into the smoke test's uploaded
        // run log. The status code is what a caller can act on anyway.
        return Err(format!(
            "onboarding submission rejected by {host}: HTTP {}. Response body withheld: \
             it may echo the request, which carries the submitter's resolved PII.",
            response.code
        ));
    }

    let _ = logging::info(&format!(
        "onboarding submitted for {} to {host} (HTTP {})",
        req.legal_name, response.code
    ));

    Ok(OnboardingResp {
        submitted,
        status_code: response.code,
        reference: extract_reference(&response.payload),
        endpoint_host: host,
    })
}

/// Turns a host error into a message that names the fix.
#[cfg(target_arch = "wasm32")]
fn format_http_error(error: hwp::HttpError, host: &str) -> String {
    match error {
        hwp::HttpError::EgressDenied(denied) => format!(
            "egress denied for {denied}: add \"{host}\" to allowedHosts in the user's \
             agent-auth grant (npm run grant)"
        ),
        hwp::HttpError::PlaceholderDenied(marker) => format!(
            "placeholder denied: {marker} — the calling user has not authorised this \
             agent to read that profile field"
        ),
        hwp::HttpError::PlaceholderUnknown(field) => format!(
            "placeholder unknown: {field} — the user profile has no such field; \
             complete the profile before onboarding"
        ),
        hwp::HttpError::PlaceholderNoUserContext => {
            "no user context: submit-onboarding must be called by an agent acting for a \
             user, not by the tenant session"
                .to_string()
        }
        hwp::HttpError::UpstreamError(reason) => format!("procurement system error: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> OnboardingReq {
        OnboardingReq {
            legal_name: "Acme GmbH".into(),
            lei: Some("529900IH9V4I3VHQVO92".into()),
            vat_number: Some("811907980".into()),
            verdict: "pass".into(),
            risk_score: 0,
        }
    }

    #[test]
    fn payload_carries_markers_not_personal_data() {
        let payload = build_payload(&request());
        let submitted_by = &payload["submitted_by"];
        assert_eq!(submitted_by["first_name"], "{{profile.first_name}}");
        assert_eq!(submitted_by["last_name"], "{{profile.last_name}}");
        assert_eq!(
            submitted_by["email"],
            "{{profile.verified_contacts.email.value}}"
        );
    }

    #[test]
    fn every_personal_field_in_the_body_is_a_marker() {
        let payload = build_payload(&request());
        let submitted_by = payload["submitted_by"].as_object().unwrap();
        for (field, value) in submitted_by {
            let value = value.as_str().unwrap_or_default();
            assert!(
                value.starts_with("{{profile.") && value.ends_with("}}"),
                "field {field} leaks a literal value: {value}"
            );
        }
    }

    #[test]
    fn company_data_is_passed_through_verbatim() {
        let payload = build_payload(&request());
        assert_eq!(payload["supplier"]["legal_name"], "Acme GmbH");
        assert_eq!(payload["supplier"]["kyb_verdict"], "pass");
        assert_eq!(payload["supplier"]["kyb_risk_score"], 0);
    }

    #[test]
    fn host_of_handles_the_shapes_a_config_value_can_take() {
        assert_eq!(host_of("https://httpbin.org/post"), "httpbin.org");
        assert_eq!(host_of("https://erp.acme.com:8443/api/v1"), "erp.acme.com");
        assert_eq!(host_of("http://erp.acme.com?x=1"), "erp.acme.com");
        assert_eq!(host_of("erp.acme.com/hook"), "erp.acme.com");
    }

    #[test]
    fn reference_extraction_tolerates_a_body_without_one() {
        assert_eq!(extract_reference(br#"{"reference":"SUP-42"}"#), "SUP-42");
        assert_eq!(extract_reference(br#"{"id":"abc"}"#), "abc");
        assert_eq!(extract_reference(br#"{"json":{}}"#), "");
        assert_eq!(extract_reference(b"not json"), "");
    }

    /// The response body is the one place plaintext PII can re-enter the
    /// enclave: the host filled the request with the submitter's real name and
    /// email, and an echoing endpoint hands it straight back. httpbin, the
    /// default endpoint, echoes on every call.
    ///
    /// Only an opaque reference id may be lifted out of that body. Nothing else
    /// from it may reach a return value, an error string, or a log line.
    #[test]
    fn nothing_personal_is_lifted_out_of_an_echoing_response() {
        let echoed = br#"{
            "json": {
                "supplier": {"legal_name": "Acme GmbH"},
                "submitted_by": {
                    "first_name": "Ada",
                    "last_name": "Lovelace",
                    "email": "ada@example.com"
                }
            },
            "headers": {"Host": "httpbin.org"},
            "id": "SUP-42"
        }"#;

        let reference = extract_reference(echoed);
        assert_eq!(reference, "SUP-42");
        for leaked in ["Ada", "Lovelace", "ada@example.com"] {
            assert!(
                !reference.contains(leaked),
                "reference must not carry {leaked} out of the enclave"
            );
        }
    }
}
