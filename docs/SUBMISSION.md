# Superteam submission — Terminal 3 / T3N trusted agent bounty

> Draft for the public Google Doc. Paste as-is, then add the screenshots where marked.

## What I built

**t3n-kyb-agent** — a supplier due-diligence (KYB) agent on the T3 Agent Developer Kit.

- Repository: `https://github.com/ErnIIk/t3n-kyb-agent` *(public)*
- Contract: `z:<tenant>:kyb-contracts` — Rust, compiled to a `wasm32-wasip2` component
- Environment: testnet

It answers the question a company asks before signing any new vendor — is this counterparty real,
active, and safe to pay? — and files the onboarding record without the buyer's contact details ever
touching the agent's process.

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

**Screenshot 1** — `npm run whoami`: tenant and agent DIDs, two separate identities.

**Screenshot 2** — `npm run deploy`: contract registered, KV maps created, config seeded.

**Screenshot 3** — `npm run grant`: the user authorising 4 functions and 3 hosts for the agent.

**Screenshot 4** — `npm run kyb`: a full KYB verdict against live GLEIF and VIES data.

**Screenshot 5** — `npm run kyb -- --submit`: onboarding filed, with the echoed body showing the
`{{profile.*}}` markers resolved by the host and not by the contract.

## Usefulness and ease of maintenance

This was the judging criterion I optimised for, so the specifics:

- **It runs with no third-party accounts.** GLEIF and VIES are free and keyless. The only
  credentials anyone needs are their own two T3N keys.
- **33 tests, none of which need credentials.** CI builds the contract, runs the suite and
  typechecks the client on every push, with no secrets configured.
- **The build fails when the grant drifts from the code.** `contract/tests/allowlist.rs` reads
  `src/session.ts` and fails if a host or function the contract uses is missing from the agent
  grant. That mismatch is otherwise invisible until runtime, where it appears as
  `host/http.egress_denied` during a demo.
- **CI builds both targets.** Host-calling code is `cfg(target_arch = "wasm32")`, so native tests
  cannot reach it. This actually bit me mid-build: a removed import kept 33 native tests green while
  the enclave path no longer compiled. CI now catches it.
- **Re-pointing the integration is a config write, not a redeploy.** The onboarding endpoint lives
  in a KV map; changing it is one environment variable, and the grant must be re-signed — deliberate,
  since changing where PII goes should need the user's signature.
- **The scoring is a table, not a black box.** Every rule that can reject a supplier is one line in
  `kyb.rs` and one entry in the returned `checks` array, because a procurement officer has to be
  able to justify a rejection to the supplier.
- **The agent is discoverable, and its card cannot lie.** `npm run register-card` publishes a public
  agent card hosted by T3N itself — no external pinning service to keep alive. It is generated from
  the same constants as the grant, so the card cannot advertise a skill the agent was never
  authorised to perform.
- **Handover docs exist.** `docs/MAINTENANCE.md` covers routine tasks, what rots on its own and how
  you find out, fixture refresh, key rotation, and the three files a new owner will actually edit.

## Bugs found

Ten issues, each with reproduction steps and the workaround, in
[`BUGS.md`](https://github.com/ErnIIk/t3n-kyb-agent/blob/main/BUGS.md). The three that cost real time —
all three are cases where copying the documented code produces something that does not work:

1. **`agent-auth-adk` puts a `Did` object where a string belongs.** The page ends with
   `const agentDid = await agentClient.authenticate(...)` and feeds that straight into the
   `agentDid` field of the grant — but `authenticate()` returns `Did { value }`, so the grant is
   written against a serialised object. Quickstart and `invoke-contract` both do `.value` correctly;
   this page is the odd one out. Copy-pasting it produces a grant that does not match the agent.

2. **`cargo test` from the walkthrough cannot run.** The testing page says to unit-test natively,
   but the reference repo you are told to clone ships `.cargo/config.toml` with
   `[build] target = "wasm32-wasip2"`, so Cargo builds the test harness as a WASM component and
   tries to execute it: `os error 193` on Windows, `Exec format error` on Linux. Needs either a note
   on the testing page or a change to the reference repo.

3. **`invoke-contract` omits `contract_version` from the `agent-auth-update` call.** The SDK's own
   internal docs say the server deserialises strictly into `contract_id` / `contract_version` /
   `function_name` and returns `Invalid action request: missing field …` otherwise. The Agent Auth
   page passes it for the same call; the invoke page does not, and the agent-side example a few
   lines below *does* — so it reads as an editing slip. Behind it sits a second gap: nothing
   documents where that version comes from. The answer is `getContractVersion(rpcUrl, contractId)`,
   which the SDK exports but no page mentions, and `"latest"` is not accepted because the server
   parses the field as SemVer.

Also: no stated source for `wit/deps/` when starting from scratch (the only way I found is copying
from `z-tenant-flight`); `T3N_API_KEY` is a secp256k1 private key rather than an API key, which
invites the wrong handling; the docs teach `executeControl("map-entry-set")` when SDK 5.10 has a
typed `maps.entrySet()`; and `use-cases/payroll-agent` is a stub that redirects out of the ADK docs.

## Running it after the challenge

<!-- Pick one before submitting and delete the other. -->

**Option A — I would like to keep running it.** I am interested in the startup program and the
listing page. The repository is public, CI is green, and the maintenance guide is written for
someone other than me.

**Option B — happy to hand it over.** The handover is a repo transfer plus one contract
re-registration under your tenant: `npm run setup` with your keys reproduces the whole deployment.
No hosted state exists outside the T3N node, and nothing in the repo is tied to my identity —
`docs/MAINTENANCE.md` documents everything a new owner touches.

## Links

- Repository: https://github.com/ErnIIk/t3n-kyb-agent
- Architecture: `docs/ARCHITECTURE.md`
- Maintenance / handover: `docs/MAINTENANCE.md`
- Bugs: `BUGS.md`
