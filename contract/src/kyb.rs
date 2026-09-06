//! run-kyb-check: the one call an agent actually makes.
//!
//! Running GLEIF and VIES in a single enclave invocation matters for more than
//! latency: the agent receives a verdict, while the intermediate lookups stay
//! inside the enclave. It also means the scoring rules live in one auditable
//! place instead of being re-implemented by every caller.

use crate::common::names_match;
use crate::gleif::EntityRecord;
use crate::vies::VatRecord;

/// Below this score the supplier is cleared automatically.
const PASS_BELOW: u32 = 20;
/// At or above this score the supplier is rejected outright.
const FAIL_AT: u32 = 50;

#[derive(serde::Deserialize, Debug)]
pub struct KybReq {
    pub legal_name: String,
    pub lei: Option<String>,
    pub country_code: Option<String>,
    pub vat_number: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Check {
    pub name: String,
    /// pass | warn | fail | skipped
    pub status: String,
    pub detail: String,
}

impl Check {
    fn new(name: &str, status: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: status.to_string(),
            detail: detail.into(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct KybVerdict {
    /// pass | review | fail
    pub verdict: String,
    /// 0-100, higher means more risk.
    pub risk_score: u32,
    pub checks: Vec<Check>,
    pub entity: EntityRecord,
    pub vat: Option<VatRecord>,
}

pub fn run_kyb_check(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: KybReq =
        serde_json::from_slice(input).map_err(|e| format!("run-kyb-check: bad input: {e}"))?;
    if req.legal_name.trim().is_empty() {
        return Err("run-kyb-check: legal_name is required".to_string());
    }

    #[cfg(target_arch = "wasm32")]
    {
        let verdict = run_wasm(req)?;
        serde_json::to_vec(&verdict).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("run_kyb_check performs I/O and only runs on the wasm32 target".to_string())
    }
}

/// Turns the two lookups into a verdict.
///
/// Pure scoring function — no host imports — so the whole decision table is
/// covered by native unit tests. Every rule that can reject a supplier is
/// something a procurement officer would have to justify, so each one adds an
/// explicit `Check` entry rather than silently moving the number.
pub fn score(req: &KybReq, entity: &EntityRecord, vat: Option<&VatRecord>) -> KybVerdict {
    let mut checks = Vec::new();
    let mut risk = 0_u32;

    if entity.found {
        checks.push(Check::new(
            "gleif_entity_found",
            "pass",
            format!("LEI {} on file", entity.lei),
        ));
    } else {
        risk += 50;
        checks.push(Check::new(
            "gleif_entity_found",
            "fail",
            "no LEI record matches this company",
        ));
    }

    if entity.found {
        if entity.entity_status.eq_ignore_ascii_case("ACTIVE") {
            checks.push(Check::new("entity_active", "pass", "entity status ACTIVE"));
        } else {
            risk += 30;
            checks.push(Check::new(
                "entity_active",
                "fail",
                format!("entity status {}", entity.entity_status),
            ));
        }

        if entity.registration_status.eq_ignore_ascii_case("ISSUED") {
            checks.push(Check::new(
                "lei_registration_current",
                "pass",
                format!("registration ISSUED, renews {}", entity.next_renewal_date),
            ));
        } else {
            risk += 20;
            checks.push(Check::new(
                "lei_registration_current",
                "warn",
                format!("registration status {}", entity.registration_status),
            ));
        }

        if names_match(&req.legal_name, &entity.legal_name) {
            checks.push(Check::new(
                "name_match",
                "pass",
                format!("matches {}", entity.legal_name),
            ));
        } else {
            risk += 15;
            checks.push(Check::new(
                "name_match",
                "warn",
                format!(
                    "requested {:?} but registry says {:?}",
                    req.legal_name, entity.legal_name
                ),
            ));
        }
    }

    match vat {
        Some(record) if record.valid => {
            checks.push(Check::new(
                "vat_valid",
                "pass",
                format!("{}{} registered", record.country_code, record.vat_number),
            ));
        }
        Some(record) => {
            risk += 25;
            checks.push(Check::new(
                "vat_valid",
                "fail",
                format!(
                    "VIES reports {}{} as not registered ({})",
                    record.country_code, record.vat_number, record.service_status
                ),
            ));
        }
        None => {
            // Not a failure, but an incomplete file is still a reason a human
            // should look before money moves.
            risk += 10;
            checks.push(Check::new("vat_valid", "skipped", "no VAT number supplied"));
        }
    }

    let risk_score = risk.min(100);
    let verdict = if risk_score >= FAIL_AT {
        "fail"
    } else if risk_score >= PASS_BELOW {
        "review"
    } else {
        "pass"
    };

    KybVerdict {
        verdict: verdict.to_string(),
        risk_score,
        checks,
        entity: entity.clone(),
        vat: vat.cloned(),
    }
}

#[cfg(target_arch = "wasm32")]
use crate::{
    gleif::{self, VerifyEntityReq},
    host::interfaces::logging,
    vies::{self, CheckVatReq},
};

#[cfg(target_arch = "wasm32")]
fn run_wasm(req: KybReq) -> Result<KybVerdict, String> {
    let entity_url = gleif::build_query_url(&VerifyEntityReq {
        lei: req.lei.clone(),
        legal_name: Some(req.legal_name.clone()),
    })?;
    let entity = gleif::fetch_entity(&entity_url, Some(req.legal_name.as_str()))?;

    // A missing VAT number is a legitimate state (non-EU supplier), so it must
    // not abort the whole check — it is scored as an incomplete file instead.
    let vat = match (req.country_code.as_deref(), req.vat_number.as_deref()) {
        (Some(country), Some(number)) if !country.is_empty() && !number.is_empty() => {
            let vat_url = vies::build_vat_url(country, number)?;
            let vat_req = CheckVatReq {
                country_code: country.to_string(),
                vat_number: number.to_string(),
            };
            Some(vies::fetch_vat(&vat_url, &vat_req)?)
        }
        _ => None,
    };

    let verdict = score(&req, &entity, vat.as_ref());
    let _ = logging::info(&format!(
        "kyb check complete: verdict={} risk={}",
        verdict.verdict, verdict.risk_score
    ));
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_entity() -> EntityRecord {
        EntityRecord {
            found: true,
            lei: "529900IH9V4I3VHQVO92".into(),
            legal_name: "Acme GmbH".into(),
            jurisdiction: "DE".into(),
            entity_status: "ACTIVE".into(),
            registration_status: "ISSUED".into(),
            next_renewal_date: "2027-01-01".into(),
            address: "Musterstrasse 1, Berlin, DE".into(),
            candidates: vec![],
        }
    }

    fn request() -> KybReq {
        KybReq {
            legal_name: "Acme GmbH".into(),
            lei: None,
            country_code: Some("DE".into()),
            vat_number: Some("811907980".into()),
        }
    }

    fn valid_vat() -> VatRecord {
        VatRecord {
            valid: true,
            country_code: "DE".into(),
            vat_number: "811907980".into(),
            service_status: "VALID".into(),
            ..Default::default()
        }
    }

    #[test]
    fn clean_supplier_passes_with_zero_risk() {
        let verdict = score(&request(), &good_entity(), Some(&valid_vat()));
        assert_eq!(verdict.verdict, "pass");
        assert_eq!(verdict.risk_score, 0);
        assert!(verdict.checks.iter().all(|c| c.status == "pass"));
    }

    #[test]
    fn unknown_entity_fails_outright() {
        let verdict = score(&request(), &EntityRecord::default(), Some(&valid_vat()));
        assert_eq!(verdict.verdict, "fail");
        assert!(verdict.risk_score >= FAIL_AT);
        // A missing entity must not also emit misleading sub-checks.
        assert!(!verdict.checks.iter().any(|c| c.name == "entity_active"));
    }

    #[test]
    fn invalid_vat_alone_lands_in_review_not_auto_reject() {
        let bad_vat = VatRecord {
            valid: false,
            service_status: "INVALID_INPUT".into(),
            ..valid_vat()
        };
        let verdict = score(&request(), &good_entity(), Some(&bad_vat));
        assert_eq!(verdict.verdict, "review");
        assert_eq!(verdict.risk_score, 25);
    }

    #[test]
    fn missing_vat_is_scored_as_an_incomplete_file() {
        let verdict = score(&request(), &good_entity(), None);
        assert_eq!(verdict.verdict, "pass");
        assert_eq!(verdict.risk_score, 10);
        assert_eq!(
            verdict
                .checks
                .iter()
                .find(|c| c.name == "vat_valid")
                .map(|c| c.status.as_str()),
            Some("skipped")
        );
    }

    #[test]
    fn dormant_entity_with_bad_vat_is_rejected() {
        let dormant = EntityRecord {
            entity_status: "INACTIVE".into(),
            registration_status: "LAPSED".into(),
            ..good_entity()
        };
        let bad_vat = VatRecord {
            valid: false,
            ..valid_vat()
        };
        let verdict = score(&request(), &dormant, Some(&bad_vat));
        assert_eq!(verdict.verdict, "fail");
        assert_eq!(verdict.risk_score, 75);
    }

    #[test]
    fn name_mismatch_is_flagged_but_punctuation_is_not() {
        let punctuated = EntityRecord {
            legal_name: "ACME G.m.b.H.".into(),
            ..good_entity()
        };
        assert_eq!(
            score(&request(), &punctuated, Some(&valid_vat())).risk_score,
            0
        );

        let other_company = EntityRecord {
            legal_name: "Globex Ltd".into(),
            ..good_entity()
        };
        let verdict = score(&request(), &other_company, Some(&valid_vat()));
        assert_eq!(verdict.risk_score, 15);
        assert_eq!(verdict.verdict, "pass");
    }

    #[test]
    fn risk_score_never_exceeds_one_hundred() {
        let worst = EntityRecord {
            found: true,
            entity_status: "INACTIVE".into(),
            registration_status: "RETIRED".into(),
            legal_name: "Globex Ltd".into(),
            ..good_entity()
        };
        let bad_vat = VatRecord {
            valid: false,
            ..valid_vat()
        };
        let verdict = score(&request(), &worst, Some(&bad_vat));
        assert!(verdict.risk_score <= 100);
    }
}
