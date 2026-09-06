//! The grant and the code must agree on which hosts this contract calls.
//!
//! `src/session.ts` sends `ALLOWED_HOSTS` to the node in the agent-auth grant.
//! If someone changes a base URL in the contract without changing that list,
//! the failure appears at runtime as `host/http.egress_denied` — after deploy,
//! against a live tenant, usually while someone is watching a demo.
//!
//! These tests make that mismatch a build failure instead. They read the
//! TypeScript source rather than duplicating the list, so there is still only
//! one place to edit when a data source is added.

use std::fs;

/// Hosts the contract's own constants say it will call.
fn contract_hosts() -> [&'static str; 3] {
    [
        z_tenant_kyb::gleif::GLEIF_HOST,
        z_tenant_kyb::vies::VIES_HOST,
        z_tenant_kyb::onboarding::DEFAULT_ONBOARDING_HOST,
    ]
}

#[test]
fn every_host_the_contract_calls_is_in_the_typescript_allowlist() {
    let session_ts = fs::read_to_string("../src/session.ts")
        .expect("src/session.ts must exist — it holds the grant's ALLOWED_HOSTS");
    let allowlist = session_ts
        .split("ALLOWED_HOSTS")
        .nth(1)
        .expect("session.ts must define ALLOWED_HOSTS");

    for host in contract_hosts() {
        assert!(
            allowlist.contains(host),
            "{host} is called by the contract but missing from ALLOWED_HOSTS in src/session.ts — \
             outbound calls to it would fail with host/http.egress_denied"
        );
    }
}

#[test]
fn every_exported_function_is_in_the_typescript_function_list() {
    let session_ts = fs::read_to_string("../src/session.ts").expect("src/session.ts must exist");
    let world_wit = fs::read_to_string("wit/world.wit").expect("wit/world.wit must exist");

    // Exported function names in WIT look like `verify-entity: func(...)`.
    let exported: Vec<String> = world_wit
        .lines()
        .filter_map(|line| line.trim().split_once(": func"))
        .map(|(name, _)| name.trim().to_string())
        .collect();

    assert!(
        exported.len() >= 4,
        "expected at least 4 exported functions, parsed {exported:?}"
    );

    for name in exported {
        assert!(
            session_ts.contains(&format!("\"{name}\"")),
            "{name} is exported by the contract but missing from CONTRACT_FUNCTIONS in \
             src/session.ts — the agent would never be granted permission to call it"
        );
    }
}
