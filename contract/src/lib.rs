//! z-tenant-kyb — KYB (Know-Your-Business) counterparty verification contract.
//!
//! An enterprise cannot sign a supplier without answering three questions:
//!   1. Is this legal entity real and currently active?      -> GLEIF Global LEI Index
//!   2. Is its VAT registration valid?                       -> EC VIES
//!   3. Can we file the onboarding record without leaking
//!      the contact person's personal data to our servers?   -> http-with-placeholders
//!
//! All three run inside the T3N confidential-compute enclave. The calling agent
//! receives a verdict, never the raw lookups; the contract itself never sees the
//! contact person's PII in plaintext.
//!
//! # Host capabilities
//!
//! Every capability this contract holds is declared in `wit/world.wit`, and
//! nothing outside that list is reachable from here:
//! ```json
//! { "host_capabilities":
//!   ["kv_store", "logging", "tenant_context", "http", "http_with_placeholders"] }
//! ```
//!
//! # Tenant setup (performed once by `npm run deploy`)
//!
//! - KV map `z:<tid>:secrets` — optional third-party API keys
//! - KV map `z:<tid>:config`  — `onboarding_url`, the procurement endpoint
//!
//! Neither GLEIF nor VIES needs an API key, so a fresh clone of this repository
//! produces a working KYB check with no third-party signup.
#![warn(clippy::style, missing_debug_implementations)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Bumped on every registration: the node rejects a re-register at a version
/// that is not strictly higher than the deployed one.
pub const CONTRACT_VERSION: &str = "0.1.2";

wit_bindgen::generate!({
    world: "tenant-kyb",
    path: "wit",
    additional_derives: [
        serde::Deserialize,
        serde::Serialize,
    ],
    generate_all,
});

// Public so the integration test in `tests/` can assert that the hosts and
// function names in the TypeScript grant still match what this contract calls.
pub mod common;
pub mod gleif;
pub mod kyb;
pub mod onboarding;
pub mod vies;

#[derive(Debug)]
struct Component;

#[cfg(target_arch = "wasm32")]
impl exports::z::tenant_kyb::contracts::Guest for Component {
    fn verify_entity(
        req: exports::z::tenant_kyb::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("verify-entity: missing input")?;
        gleif::verify_entity(&input)
    }

    fn check_vat(req: exports::z::tenant_kyb::contracts::GenericInput) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("check-vat: missing input")?;
        vies::check_vat(&input)
    }

    fn run_kyb_check(
        req: exports::z::tenant_kyb::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("run-kyb-check: missing input")?;
        kyb::run_kyb_check(&input)
    }

    fn submit_onboarding(
        req: exports::z::tenant_kyb::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("submit-onboarding: missing input")?;
        onboarding::submit_onboarding(&input)
    }
}

#[cfg(target_arch = "wasm32")]
export!(Component);

#[cfg(test)]
mod tests {
    use super::CONTRACT_VERSION;

    #[test]
    fn contract_version_is_semver() {
        let parts: Vec<&str> = CONTRACT_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "CONTRACT_VERSION must be MAJOR.MINOR.PATCH");
        for part in parts {
            assert!(part.parse::<u32>().is_ok(), "each part must be numeric");
        }
    }
}
