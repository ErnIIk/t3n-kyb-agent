# t3n-kyb-agent

A supplier due-diligence agent built on [Terminal 3](https://terminal3.io)'s Agent Developer Kit.

It answers the question every company asks before it signs a new vendor — **is this counterparty
real, active, and safe to pay?** — and files the onboarding record without the buyer's contact
details ever touching the agent's process.

```
$ npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980

agent   : did:t3n:9f2a...
contract: z:1a2b...:kyb-contracts
supplier: Deutsche Bank Aktiengesellschaft

verdict : PASS (risk 0/100)
entity  : Deutsche Bank Aktiengesellschaft — LEI 529900IH9V4I3VHQVO92
address : 23-25, avenue Franklin Delano Roosevelt, 75008, Paris, FR
checks  :
  [PASS] gleif_entity_found      — LEI 529900IH9V4I3VHQVO92 on file
  [PASS] entity_active           — entity status ACTIVE
  [PASS] lei_registration_current— registration ISSUED, renews 2027-05-30
  [PASS] name_match              — matches Deutsche Bank Aktiengesellschaft
  [PASS] vat_valid               — DE811907980 registered
```

## Verify it without an account

The whole build can be checked before you claim a single key — no T3N account, no third-party
signup, nothing to configure:

```bash
npm ci && npm run test:contract && npm run build:contract && npm run typecheck
```

That compiles the TEE contract to a WASM component, runs 33 tests, and typechecks the client. The
same four commands are what CI runs on every push.

## Why this, on this platform

KYB is a good fit for a confidential-compute platform for a specific reason: the *company* data is
public, but the *people* data attached to it is not. A supplier onboarding form carries the name,
date of birth and email of a real contact person, and that record usually ends up copied across a
procurement system, a CRM, an email thread and three log files on the way.

On T3N the split is enforced by the runtime rather than by discipline:

| What | Where it lives | Who can see it |
|---|---|---|
| Company lookups (GLEIF, VIES) | inside the enclave | the contract, then the agent gets a verdict |
| Contact person's name / DOB / email | never leaves the user's profile | resolved inside the enclave at dispatch |
| Third-party API keys | `z:<tid>:secrets` KV map | only the contract, inside the TEE |
| What the agent may call | the user's signed grant | the node enforces it per function and per host |

Terminal 3's own docs list **B2B procurement** as a target use case for delegated agents; this is a
working implementation of it.

## What it does

Four functions, exported from one WASM contract that runs inside the enclave:

| Function | Source | Carries PII | Purpose |
|---|---|---|---|
| `verify-entity` | GLEIF Global LEI Index | no | Is the legal entity registered and active? |
| `check-vat` | EC VIES | no | Is the VAT number valid right now? |
| `run-kyb-check` | both, in one call | no | Scored verdict: `pass` / `review` / `fail` |
| `submit-onboarding` | your procurement endpoint | **yes, via placeholders** | File the supplier record |

Both data sources are public and need **no API key and no signup**, so a reviewer can clone this
repository and get a real verdict against real registry data with only their T3N keys.

### The scoring is a table, not a black box

Every rule that can reject a supplier is one line in [`contract/src/kyb.rs`](contract/src/kyb.rs)
and one entry in the returned `checks` array, because a procurement officer has to be able to
justify a rejection to the supplier:

| Finding | Risk added |
|---|---|
| No LEI record matches | +50 |
| Entity status not `ACTIVE` | +30 |
| VAT number not registered | +25 |
| LEI registration not `ISSUED` (lapsed, retired) | +20 |
| Registry name differs from the requested name | +15 |
| No VAT number supplied (incomplete file) | +10 |

`< 20` passes, `20–49` goes to human review, `>= 50` is rejected. The whole table is covered by
unit tests.

## How the PII protection actually works

`submit-onboarding` builds this body:

```jsonc
{
  "supplier": { "legal_name": "Acme GmbH", "kyb_verdict": "pass" },
  "submitted_by": {
    "first_name": "{{profile.first_name}}",
    "last_name":  "{{profile.last_name}}",
    "email":      "{{profile.verified_contacts.email.value}}"
  }
}
```

Those markers are literal strings in the WASM module. The host substitutes the real values inside
the enclave, at dispatch time, and only if the calling user has authorised *this* agent for *this*
function and *this* destination host. The contract cannot read them, cannot log them, and cannot
send them anywhere else — a redirect to a different host fails with `host/http.egress_denied`
before any substitution happens.

A test enforces this so a future edit cannot quietly inline a real value:

```rust
// every field in submitted_by must be a {{profile.*}} marker
assert!(value.starts_with("{{profile.") && value.ends_with("}}"),
        "field {field} leaks a literal value: {value}");
```

## Quick start

Prerequisites: Node 20+, Rust with `wasm32-wasip2`, and **two** keys from the
[claim page](https://docs.terminal3.io/developers/adk/get-started/prerequisites/request-test-tokens)
— one for you, one for the agent. They are separate identities with separate credits; reusing one
key for both defeats the delegation model this repository exists to demonstrate.

```bash
git clone https://github.com/ErnIIk/t3n-kyb-agent && cd t3n-kyb-agent
npm install
rustup target add wasm32-wasip2

cp .env.example .env       # paste both keys

npm run quickstart         # the documented Quickstart: prints your tenant DID
npm run whoami             # verifies both identities before anything is deployed
```

`quickstart` prints your tenant DID — put it in `.env` as `T3N_TENANT_DID`, then:

```bash
npm run setup              # build the contract, register it, create maps, grant the agent
npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980
npm run kyb -- --name "Acme GmbH" --country DE --vat 811907980 --submit
```

`npm run setup` is `build:contract` + `deploy` + `grant`. Every step is idempotent — re-running it
after a failure is safe, and the only thing that ever needs a manual bump is the contract version
in `contract/Cargo.toml`.

Optionally, make the agent discoverable to other agents and services:

```bash
npm run register-card      # publishes a public agent card, hosted by T3N itself
```

The card is generated from the same constants as the grant, so it cannot advertise a skill the
agent has not been authorised to perform.

## Layout

```
contract/            the TEE contract — Rust, compiled to a WASI P2 component
  wit/world.wit      the capability list: nothing outside it is reachable
  src/gleif.rs       GLEIF lookup + response parsing
  src/vies.rs        VIES lookup + response parsing
  src/kyb.rs         orchestration and the scoring table
  src/onboarding.rs  the placeholder-templated submission
  src/common.rs      KV access, input validation, URL encoding
  tests/             fixtures from the live APIs + the grant/code consistency test
src/                 the TypeScript side
  session.ts         auth for both identities, and the grant's allowlists
  quickstart.ts      the documented Quickstart, kept runnable on its own
  whoami.ts          check credentials before deploying
  deploy.ts          register the contract, create KV maps, seed config
  grant.ts           the user's signed authorisation of the agent
  register-card.ts   publish the public agent card (discovery)
  agent.ts           the agent itself
```

## Maintenance

This was built to be handed over, so the things that rot are documented rather than implied — see
[docs/MAINTENANCE.md](docs/MAINTENANCE.md) for the full list. The short version:

- **Adding a data source** means editing three places, and CI fails if you miss one. The
  `allowlist` test reads `src/session.ts` and asserts every host and function the contract uses is
  in the grant.
- **Changing where onboarding records go** is a KV write, not a redeploy:
  `ONBOARDING_URL=... npm run deploy` (the host must also be in the grant).
- **Redeploying** requires bumping `version` in `contract/Cargo.toml`; the node rejects a
  re-register at the same version, and `deploy.ts` says so explicitly when it happens.
- **No credentials are needed to work on this.** CI builds the contract, runs 33 tests and
  typechecks the client without any T3N key.
- **It tells you when it stops working.** [A weekly workflow](.github/workflows/smoke.yml) runs the
  real agent against testnet and fails loudly if it breaks, checking the public registries in a
  separate job so you can tell upstream problems from T3N ones. Without secrets configured it skips
  and passes, so a fork never fails a run it cannot pass.

This agent is offered to Terminal 3 to maintain; taking it over is a 20-minute process, written out
in [docs/HANDOVER.md](docs/HANDOVER.md).
[docs/WALKTHROUGH.md](docs/WALKTHROUGH.md) maps every documented Quickstart and Walkthrough step to
the file that performs it.

## Bugs and docs feedback

Everything that cost time during this build is written up in [BUGS.md](BUGS.md), with reproduction
steps and the fix that worked.

## Security notes

- `.env` is gitignored; keys are read from the environment and never written to a file by any script.
- Both VIES path segments and the GLEIF query are validated and encoded before they reach a URL —
  see the path-traversal tests in `contract/src/common.rs`.
- The contract logs company identifiers only. No log line in this repository can contain personal data.
- The KV maps are created with an explicit `readers` list. Omitting it silently produces a
  deny-all map whose own contract cannot read it back.

## License

MIT
