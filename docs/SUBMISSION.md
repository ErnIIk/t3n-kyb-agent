# Superteam submission — Terminal 3 / T3N trusted agent bounty

> Draft for the public Google Doc. Paste as-is and add the screenshots where marked.

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

## What is different about this submission

Six things a reviewer will not find in a typical build, each of which exists for a reason rather
than for decoration:

1. **A test that fails when the permission grant drifts from the code.** Adding an outbound host to
   the contract without adding it to the agent grant is invisible until runtime, where it surfaces as
   `host/http.egress_denied` — usually during a demo. `contract/tests/allowlist.rs` reads the
   TypeScript grant and fails the build instead.
2. **CI that builds both targets, because native tests are not enough.** Host-calling code is behind
   `cfg(target_arch = "wasm32")`. Mid-build, a removed import left every native test green while the
   enclave path no longer compiled. That is now a CI failure, not a surprise at deploy time.
3. **A weekly live smoke test.** The agent runs against testnet every Monday and fails loudly when it
   breaks, with upstream registries checked in a separate job so an outage at GLEIF is
   distinguishable from a problem at Terminal 3. "Running post challenge" as a workflow, not a promise.
4. **An agent card generated from the grant's own constants.** The documented flow produces a
   hand-maintained JSON file that drifts; here the card cannot advertise a skill the agent was never
   authorised to perform.
5. **Bugs verified against the SDK's types, not just experienced.** Each entry names the file and
   line in `index.d.ts` that contradicts the documentation — including a probable single root cause:
   the docs were validated against SDK 3.x while npm serves 5.10.0.
6. **The documented demos are covered by tests.** Both commands in the README are pinned by
   regression tests against captured live registry responses, so the README, the weekly smoke test
   and the code cannot drift apart. That check earned its place immediately: it exposed that
   searching GLEIF for "Acme GmbH" returns a *retired* company first, which scored 65 and made the
   documented `--submit` demo refuse to run. Records are now ranked — name agreement above liveness
   above a current registration — and the rows that lose come back as `candidates` so a human can
   overrule the choice.

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
8. `npm run test:contract` — 39 tests passing with no credentials configured

## Usefulness and ease of maintenance

This was the judging criterion I optimised for, so the specifics rather than adjectives:

- **It runs with no third-party accounts.** GLEIF and VIES are free and keyless. The only credentials
  anyone needs are their own two T3N keys.
- **39 tests, none of which need credentials.** CI builds the contract, runs the suite and typechecks
  the client on every push, with no secrets configured.
- **The build fails when the grant drifts from the code.** `contract/tests/allowlist.rs` reads
  `src/session.ts` and fails if a host or function the contract uses is missing from the agent grant.
  That mismatch is otherwise invisible until runtime, where it appears as `host/http.egress_denied`
  in front of whoever you were demonstrating to.
- **CI builds both targets.** Host-calling code is `cfg(target_arch = "wasm32")`, so native tests
  cannot reach it. This bit me mid-build: a removed import kept every native test green while the
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

Eleven issues, each with reproduction steps and the workaround, in
[`BUGS.md`](https://github.com/ErnIIk/t3n-kyb-agent/blob/main/BUGS.md).

The one I would fix first costs one sentence. **How the agent gets its own key is documented on
exactly one page — the one about organisation-owned agents.** Agent Auth tells you to get the
agent a key "from the same claim page you used for your own"; that page documents a single sign-in
issuing a single key and warns it "is shown once… there's no way to view it again". Read in the
order the docs present them, the conclusion is that keys are one per account. The real answer —
"issues a fresh key together with metered test credits every time you visit" — appears only on
[Register an Organization-owned Agent](https://docs.terminal3.io/developers/agents/provision-org-agent),
which the public-agent walkthrough never links to.

It is worth fixing despite its size because both wrong answers fail quietly: reusing the tenant key
gives a working handshake and a successful grant, since an identity may authorise itself, so the
delegation demonstrates nothing while appearing to work — and that is the one property the platform
exists to provide.

**All eleven were checked against your own twelve-row known-pitfalls table** in
[Using AI Coding Assistants](https://docs.terminal3.io/developers/adk/support/ai-coding-assistants),
and none of them duplicates a row in it. That table covers runtime symptoms hit while following the
docs correctly; this report covers places where the documentation is wrong, missing, or contradicts
another page. Two findings are cases of the docs disagreeing with themselves — the skill file says
"never construct the DID, read `did.value`", while the Agent Auth page passes the `Did` object
straight into the grant.

The three that cost the most time are all cases where copying the documented code produces something
that does not work:

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

**I would like to hand it over to Terminal 3 to maintain.**

The full process is in
[`docs/HANDOVER.md`](https://github.com/ErnIIk/t3n-kyb-agent/blob/main/docs/HANDOVER.md). In short:
transfer the repository, claim two keys, `npm run setup`, `npm run register-card`, verify with one
command, and add three repository secrets to switch on the weekly smoke test. About 20 minutes, most
of it a Rust build.

What makes it that short is what you are not inheriting: there is no hosted service, no database, no
cron host, no paid dependency, and nothing tied to my identity — the tenant DID simply follows
whichever key runs the deploy, and both data sources are public and keyless. The maintenance guide is
written for a stranger rather than for me, and `docs/HANDOVER.md` includes an honest table of the
ongoing load: an SDK major bump is the one thing likely to need real attention, which is why every
handshake and `execute` call goes through a single file.

Until the handover completes, the agent stays deployed on testnet and I will keep it running — the
weekly smoke test means a break is visible rather than discovered later.

I am happy to walk someone through it or answer questions during the transfer.

## Social post (bonus)

> Built a KYB agent on @terminal3io's ADK.
>
> It checks a supplier against the GLEIF LEI index and EU VIES inside a TEE contract, scores a
> pass/review/fail verdict, then files the onboarding record — while the contact person's name and
> email are resolved inside the enclave and never touch my process.
>
> Both registries are public and keyless, so you can clone it and get a real verdict with just your
> own T3N keys. 39 tests, none of which need credentials.
>
> https://github.com/ErnIIk/t3n-kyb-agent

Follow-up post:

> Eleven bugs and docs issues found on the way, each with a reproduction — including no documented
> way to claim the second key every agent needs, a docs page that hands a `Did` object where a string
> belongs, and a `cargo test` that cannot run because the reference repo pins the WASM target.
>
> Probable root cause: the docs were validated against SDK 3.x, npm serves 5.10.0.

## Links

- Repository: https://github.com/ErnIIk/t3n-kyb-agent
- Quickstart & Walkthrough evidence: `docs/WALKTHROUGH.md`
- Architecture: `docs/ARCHITECTURE.md`
- Maintenance: `docs/MAINTENANCE.md`
- Handover: `docs/HANDOVER.md`
- Bugs: `BUGS.md`
