# Superteam submission — Terminal 3 / T3N trusted agent bounty

> Draft for the public Google Doc. Paste as-is, add the screenshots where marked, and pick one of
> the two options in "Running it after the challenge".

## What I built

**t3n-kyb-agent** — a supplier due-diligence (KYB) agent on the T3 Agent Developer Kit.

- Repository: `https://github.com/ErnIIk/t3n-kyb-agent` *(public)*
- Contract: `z:<tenant>:kyb-contracts` — Rust, compiled to a `wasm32-wasip2` component
- Agent card: `<node>/api/agent-card/<agent-did>` *(published, hosted by T3N)*
- Environment: testnet

It answers the question a company asks before signing any new vendor — is this counterparty real,
active, and safe to pay? — and files the onboarding record without the buyer's contact details ever
touching the agent's process.

## Scope checklist

| Requirement | Status | Where |
|---|---|---|
| Sign up via SSO | done | — |
| Obtain DID & API key | done | two keys claimed: one tenant, one agent |
| Complete the Quickstart | done | `src/quickstart.ts`, `npm run quickstart` — Screenshot 1 |
| Complete the Walkthrough | done | all 5 steps mapped in `docs/WALKTHROUGH.md` |
| Enterprise agent, useful | done | KYB — GLEIF + VIES + PII-safe onboarding |
| Easy to maintain | done | see the section below; `docs/MAINTENANCE.md` |
| Running post challenge | done | weekly live smoke test, `.github/workflows/smoke.yml` |
| Continue running or hand over | answered below | full process in `docs/HANDOVER.md` |
| Public GitHub repo | done | link above |
| Screenshots | below | also in `screenshots/` |
| Bugs faced | 10, reproducible | `BUGS.md` |

## Why KYB, on this platform specifically

Terminal 3's own use-case page lists **B2B procurement** as a target for delegated agents. KYB fits
confidential compute for a concrete reason: the *company* data is public, but the *people* data
attached to it is not. A supplier onboarding record carries a real person's name, date of birth and
email, and in a normal stack that record gets copied into a procurement system, a CRM, an email
thread and several log files on the way.

Here the split is enforced by the runtime instead of by discipline:

| What | Where it lives | Who can see it |
|---|---|---|
| Company lookups (GLEIF, VIES) | inside the enclave | contract; the agent only gets a verdict |
| Contact person's name / DOB / email | the user's profile | resolved inside the enclave at dispatch |
| Third-party API keys | `z:<tid>:secrets` KV map | only the contract, inside the TEE |
| What the agent may do | the user's signed grant | enforced per function and per host |

## The agent

Four functions exported from one TEE contract:

| Function | Data source | Carries PII | Purpose |
|---|---|---|---|
| `verify-entity` | GLEIF Global LEI Index | no | Is the legal entity registered and active? |
| `check-vat` | EC VIES | no | Is the VAT number valid right now? |
| `run-kyb-check` | both, one invocation | no | Scored verdict: pass / review / fail |
| `submit-onboarding` | procurement endpoint | **yes — via `{{profile.*}}`** | File the supplier record |

Both data sources are public and need **no API key and no signup**, so anyone reviewing this can
clone the repo and get a real verdict against real registry data using only their own T3N keys.

The scoring is a table, not a black box — every rule that can reject a supplier is one line in
`kyb.rs` and one entry in the returned `checks` array, because a procurement officer has to be able
to justify a rejection to the supplier.

## Screenshots

1. `npm run quickstart` — the documented Quickstart: `Connected as: did:t3n:…`
2. `npm run whoami` — tenant and agent as two separate identities
3. `npm run deploy` — contract registered, KV maps created, config seeded
4. `npm run grant` — the user authorising 4 functions and 3 hosts
5. `npm run register-card` — public agent card published and read back
6. `npm run kyb` — a full KYB verdict against live GLEIF and VIES data
7. `npm run kyb -- --submit` — onboarding filed; the echoed body shows the `{{profile.*}}` markers
   resolved by the host and never by the contract
8. `npm run test:contract` — 33 tests passing with no credentials configured

## Usefulness and ease of maintenance

This was the judging criterion I optimised for, so the specifics rather than adjectives:

- **It runs with no third-party accounts.** GLEIF and VIES are free and keyless. The only credentials
  anyone needs are their own two T3N keys.
- **33 tests, none of which need credentials.** CI builds the contract, runs the suite and typechecks
  the client on every push, with no secrets configured.
- **The build fails when the grant drifts from the code.** `contract/tests/allowlist.rs` reads
  `src/session.ts` and fails if a host or function the contract uses is missing from the agent grant.
  That mismatch is otherwise invisible until runtime, where it appears as `host/http.egress_denied`
  in front of whoever you were demonstrating to.
- **CI builds both targets.** Host-calling code is `cfg(target_arch = "wasm32")`, so native tests
  cannot reach it. This bit me mid-build: a removed import kept 33 native tests green while the
  enclave path no longer compiled. CI now catches it.
- **It tells you when it stops working.** A weekly workflow runs the real agent against testnet and
  fails loudly if it breaks, with the public registries checked in a separate job so upstream
  outages are distinguishable from T3N problems.
- **Re-pointing the integration is a config write, not a redeploy.** The onboarding endpoint lives in
  a KV map; changing it is one environment variable, and the grant must be re-signed — deliberate,
  since changing where PII goes should need the user's signature.
- **The agent is discoverable, and its card cannot lie.** `npm run register-card` publishes a public
  agent card hosted by T3N itself — no external pinning service to keep alive. It is generated from
  the same constants as the grant, so the card cannot advertise a skill the agent was never
  authorised to perform.
- **All T3N auth lives in one file.** `src/session.ts` is the only place that touches handshake,
  authentication and version resolution — which matters, because the SDK is the part most likely to
  move (see bug 9).

## Bugs found

Ten issues, each with reproduction steps and the workaround, in
[`BUGS.md`](https://github.com/ErnIIk/t3n-kyb-agent/blob/main/BUGS.md). The three that cost real time
are all cases where copying the documented code produces something that does not work:

1. **`agent-auth-adk` puts a `Did` object where a string belongs.** The page ends with
   `const agentDid = await agentClient.authenticate(...)` and feeds that straight into the `agentDid`
   field of the grant — but `authenticate()` returns `Did { value }`, so the grant is written against
   a serialised object. Quickstart and `invoke-contract` both do `.value` correctly; this page is the
   odd one out.

2. **`cargo test` from the walkthrough cannot run.** The testing page says to unit-test natively, but
   the reference repo you are told to clone ships `.cargo/config.toml` with
   `[build] target = "wasm32-wasip2"`, so Cargo builds the test harness as a WASM component and tries
   to execute it: `os error 193` on Windows, `Exec format error` on Linux.

3. **`invoke-contract` omits `contract_version` from the `agent-auth-update` call.** The SDK's own
   internal docs say the server deserialises strictly into `contract_id` / `contract_version` /
   `function_name` and returns `Invalid action request: missing field …` otherwise. The Agent Auth
   page passes it for the same call; the invoke page does not. Behind it sits a second gap: nothing
   documents where that version comes from. The answer is `getContractVersion(rpcUrl, contractId)`,
   which the SDK exports but no page mentions, and `"latest"` is not accepted because the server
   parses the field as SemVer.

A likely single cause for several of these: the changelog says the docs were validated against SDK
`3.5–3.11`, while `npm install` today serves **5.10.0**. Stating the SDK version each page was
verified against would let a reader know when to check the types before trusting a snippet.

Also: no stated source for `wit/deps/` when starting from scratch (the only way I found is copying
from `z-tenant-flight`); `T3N_API_KEY` is a secp256k1 private key rather than an API key, which
invites the wrong handling; the docs teach `executeControl("map-entry-set")` when SDK 5.10 has a
typed `maps.entrySet()`; the agent-card flow is documented CLI-only though the SDK exposes it
programmatically; and `use-cases/payroll-agent` is a stub that redirects out of the ADK docs.

Worth saying, since a bug list alone is a distorted picture: the WASI P2 build worked exactly as
documented with no `cargo-component`, the `readers` footgun on `maps.create` is warned about in both
the docs and the SDK's JSDoc, the `common-errors` page matched the real error strings, and the
`http-with-placeholders` error variants map one-to-one to the WIT.

## Running it after the challenge

<!-- Pick one before submitting and delete the other. -->

**Option A — I would like to keep running it,** and I am interested in the startup program and the
listing page. The repository is public, CI is green without credentials, the weekly smoke test covers
the live path, and the maintenance guide is written for someone other than me.

**Option B — happy to hand it over.** The full process is in
[`docs/HANDOVER.md`](https://github.com/ErnIIk/t3n-kyb-agent/blob/main/docs/HANDOVER.md): transfer the
repo, claim two keys, `npm run setup`, `npm run register-card`, verify with one command, add three
repository secrets to enable the weekly smoke test. About 20 minutes, most of it a Rust build.

There is no hosted service, no database, no cron host, no paid dependency, and nothing tied to my
identity — the tenant DID follows whichever key runs the deploy. That is what makes the handover
short, and those are the same design choices the "ease of maintenance" criterion asks for.

## Links

- Repository: https://github.com/ErnIIk/t3n-kyb-agent
- Quickstart & Walkthrough evidence: `docs/WALKTHROUGH.md`
- Architecture: `docs/ARCHITECTURE.md`
- Maintenance: `docs/MAINTENANCE.md`
- Handover: `docs/HANDOVER.md`
- Bugs: `BUGS.md`
