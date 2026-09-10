# Bugs and documentation issues found while building this

Everything below was hit while building `t3n-kyb-agent` against the refreshed ADK docs
(docs as of 2026-09-06, Windows 11 / Node 24.14 / Rust 1.98). This project runs on
`@terminal3/t3n-sdk@5.2.0`, the version Terminal 3 recommends; issue 12 is why, and covers what
happens on 5.10 and later.
Each entry has the reproduction, what actually happens, and the workaround that unblocked me.

Ordered by how much time they cost.

---

## 1. `agent-auth-adk` uses the DID object where a string is required, and copy-paste produces a broken grant

**Severity: high** (the copied code runs, then the grant silently does not match the agent)

[Agent Auth](https://docs.terminal3.io/developers/adk/overview/agent-auth-adk) ends the
authentication snippet with:

```typescript
await agentClient.handshake();
const agentDid = await agentClient.authenticate(createEthAuthInput(agentAddress));
```

and then feeds that same `agentDid` straight into the grant:

```typescript
input: { agents: [{ agentDid: agentDid, scripts: [...] }] }
```

But `authenticate()` returns `Did`, not `string`:

```typescript
// node_modules/@terminal3/t3n-sdk/dist/index.d.ts:257
interface Did { readonly value: string; toString(): string; }
```

So `agentDid` is an object, and the grant is written against a serialised object rather than the
`did:t3n:...` string. Two neighbouring pages get this right:
[Quickstart](https://docs.terminal3.io/developers/adk/get-started/quickstart) does
`const tenantDid = did.value;` and
[Invoke your TEE contract](https://docs.terminal3.io/developers/adk/get-started/walkthrough/invoke-contract)
does `const agentDid = agentAuth.value;`, so the Agent Auth page is the odd one out.

**Fix:** make the Agent Auth page read `const agentDid = (await agentClient.authenticate(...)).value;`

**What I did:** `src/session.ts` normalises it defensively, because the return type is easy to get
wrong twice:

```typescript
const did = typeof auth === "string" ? auth : auth.value;
```

---

## 2. `cargo test` from the walkthrough cannot run, because the reference repo pins the WASM target

**Severity: high** (the documented command fails on a clean checkout)

[Test your TEE contract](https://docs.terminal3.io/developers/adk/get-started/walkthrough/test)
tells you to unit-test natively with `cargo test`, and correctly explains that host-calling
functions only run on `wasm32`. But
[Write your TEE contract](https://docs.terminal3.io/developers/adk/get-started/walkthrough/write-contract)
tells you to clone `Terminal-3/z-tenant-flight`, and that repository ships:

```toml
# .cargo/config.toml
[build]
target = "wasm32-wasip2"
```

which makes `cargo test` build the test harness *as a WASM component* and then try to execute it:

```
$ cargo test
error: test failed, to rerun pass `--lib`

Caused by:
  could not execute process .../target/wasm32-wasip2/debug/deps/z_tenant_kyb-*.wasm (never executed)

Caused by:
  %1 is not a valid Win32 application. (os error 193)
```

Same failure on Linux (`Exec format error`).

There is a second-order trap behind this one, which cost me a further round: `.cargo/config.toml` is
resolved from the **current working directory**, not from `--manifest-path`. So the same crate builds
differently depending on where you stand:

```bash
cd contract && cargo build --release          # wasm32-wasip2 — config applies
cargo build --release --manifest-path contract/Cargo.toml   # HOST target — no .wasm produced
```

The second command finishes with `Finished release profile`, having quietly built a native `.rlib`
and no component at all. Any npm script or CI step that builds from the repo root hits this, and the
failure only appears one step later as a missing `.wasm` file.

**Fix:** either mention the target override on the testing page, or drop `[build] target` from the
reference repo in favour of passing `--target wasm32-wasip2` in the documented build command. The
latter is safer, since it does not depend on where cargo is invoked from.

**What I did:** every script runs from the repo root, where the contract's cargo config does not
apply, and states the target explicitly when it needs one:

```jsonc
"build:contract": "cargo build --release --manifest-path contract/Cargo.toml --target wasm32-wasip2",
"test:contract":  "cargo test  --manifest-path contract/Cargo.toml",  // host default
```

This also keeps the test script portable. An earlier version pinned
`x86_64-pc-windows-msvc`, which would have failed for any reviewer on macOS or Linux.

---

## 3. `invoke-contract` omits `contract_version`, which the server rejects outright

**Severity: high** (the documented call fails with a 400 on the field the example is missing)

[Invoke your TEE contract](https://docs.terminal3.io/developers/adk/get-started/walkthrough/invoke-contract)
shows the user authorising the agent like this:

```typescript
await userClient.execute({
  contract_id: "tee:user/contracts",
  function_name: "agent-auth-update",
  input: { agents: [...] },
});
```

No `contract_version`. The SDK's own documentation for the request builder says that is not
optional:

> The server deserialises strictly into `contract_id` / `contract_version` / `function_name` —
> sending `contract` / `version` / `function` produces `Invalid action request: missing field …`
> 400s.
> — `index.d.ts:3559-3568`

[Agent Auth](https://docs.terminal3.io/developers/adk/overview/agent-auth-adk) gets it right and
passes `contract_version: userContractVersion` for the same call, so the two pages disagree about
the same request. The agent-side `executeAndDecode` example on the invoke page *does* include
`contract_version: scriptVersion`, which makes the omission look like an editing slip rather than a
deliberate difference.

There is a second gap behind it: neither page says where `userContractVersion` comes from. The
answer is `getContractVersion(rpcUrl, contractId)`, which is exported from the SDK but appears in no
documentation page, and a literal `"latest"` cannot be used, because the server parses the field as
SemVer.

**Fix:** add `contract_version` to the invoke-contract example, and document `getContractVersion` as
the supported way to obtain it.

**What I did:** `src/session.ts` wraps it once, and every `execute` call in this repository resolves
the version through it:

```typescript
export async function resolveContractVersion(contractId: string): Promise<string> {
  return getContractVersion(getNodeUrl(), contractId);
}
```

---

## 4. There is no stated source for `wit/deps/` when starting a contract from scratch

**Severity: medium** (blocks anyone not copying the reference repo wholesale)

`world.wit` must import `host:tenant/tenant-context@1.0.0` and `host:interfaces/*@2.1.0`, and
[Capabilities come from your WIT imports](https://docs.terminal3.io/developers/adk/tips/capabilities-from-wit-import)
explains what those grant. Nothing states where the interface definitions themselves come from:
there is no crates.io package, no npm package, and no documented download. `wit-bindgen` fails
without them.

The only working source I found is the reference repo:

```
z-tenant-flight/wit/deps/host-interfaces-2.1.0/package.wit
z-tenant-flight/wit/deps/host-outbox-1.0.0/package.wit
z-tenant-flight/wit/deps/host-tenant-1.0.0/package.wit
```

**Fix:** publish the host WIT packages somewhere versioned and link it from the WIT page, or say
plainly on the write-contract page that `wit/deps/` must be copied from `z-tenant-flight`.

**What I did:** copied `wit/deps/` from the reference repo. It is vendored in this repository so a
clone builds without a second checkout.

---

## 5. `T3N_API_KEY` is not an API key but a secp256k1 private key

**Severity: medium** (a naming problem with a security consequence)

The Quickstart says "Get your API key" and "Your key is shown once", then has you export it as
`T3N_API_KEY`. [Register a Public Agent](https://docs.terminal3.io/developers/agents/register-agent)
is where it becomes explicit:

```bash
export T3N_API_KEY="0x<private_key>"
```

It is a signing key: `metamask_sign(address, undefined, T3N_API_KEY)` signs the SIWE challenge with
it. Calling it an "API key" invites the handling an API key usually gets: pasted into a shared
`.env`, a CI variable, a Slack message, a support ticket. Anyone holding it can authenticate as
that identity outright.

**Fix:** name it `T3N_PRIVATE_KEY` (keeping the old name as an alias), and say on the claim page
that it is a private key, not a bearer token.

**Related, same page:** the claim step gives you one key, but every agent needs its own key with its
own credits. That only becomes clear several pages later, at `register-agent` step 2. Saying it on
the claim page would save a round trip, and with it an `InsufficientCreditError` that reads like a billing
problem rather than "you used the wrong identity".

---

## 6. Docs teach `executeControl("map-entry-set")` when the SDK has a typed helper

**Severity: low** (works, but is the harder of two paths)

[Seed API key into secrets map](https://docs.terminal3.io/developers/adk/tips/seed-api-key)
documents:

```typescript
await tenant.executeControl("map-entry-set", {
  map_name: tenant.canonicalName("secrets"),
  key: "duffel_api_key",
  value: process.env.DUFFEL_API_KEY!,
});
```

SDK 5.10 has a first-class method that builds the canonical name itself:

```typescript
// index.d.ts:6369
entrySet(tail: string, key: string, value: string, opts?: TenantTargetOptions): Promise<void>;
```

so the same write is `await tenant.maps.entrySet("secrets", "duffel_api_key", key)`. The typed path
also avoids the `map_name` / `tail` confusion that the "canonical map name invalid" error is about.

**Fix:** show `maps.entrySet` as the primary example; keep `executeControl` as the escape hatch.

---

## 7. `use-cases/payroll-agent` is a dead end from inside the ADK docs

**Severity: low** (documentation navigation)

`llms.txt` lists **Payroll Agent** under ADK use cases, but the page contains only a pointer to
`/t3n/use-cases/delegate-access-to-agent#payroll`. A developer arriving from the ADK walkthrough
looking for a worked enterprise example finds a redirect instead.

The destination page is good: it has Enterprise, **B2B Procurement**, Payroll and Individual
sections, which makes the stub more of a shame. Inlining even a short summary with a link would
keep the ADK reading path intact.

---

## 8. Minor: Quickstart installs `tsx` as a production dependency

**Severity: cosmetic**

```bash
npm install @terminal3/t3n-sdk tsx
```

`tsx` is a dev tool; this puts it in `dependencies`. `npm install @terminal3/t3n-sdk && npm install -D tsx`
keeps a deployed agent's dependency tree honest.

---

## 9. Root cause of several of the above: the docs target SDK 3.x, npm serves 5.10.0

**Severity: medium** (a versioning gap that quietly generates the mismatches above)

The [Changelog](https://docs.terminal3.io/developers/adk/changelog) says the SDK versions referenced
by hackathon projects are `3.5.2`, `3.9.0` and `3.11.0`, and adds:

> We haven't cross-checked these against an official release history yet, so we're not listing
> per-version changes here until we can confirm them.

Meanwhile `npm install @terminal3/t3n-sdk` today installs **5.10.0**. That is two majors ahead of
anything the docs were validated against, which is a plausible single cause for issues 1, 3 and 6 in
this list: the `Did` return shape, the missing `contract_version`, and `executeControl` being shown
where a typed `maps.entrySet` now exists.

**Fix:** state the SDK version each documentation page was verified against, even approximately. A
reader who knows the page targets 3.x will check the types before trusting a snippet; right now the
pages read as current.

---

## 10. The agent-card flow has a programmatic API that no page mentions

**Severity: low** (a CLI-only story for something the SDK does properly)

[Register a Public Agent](https://docs.terminal3.io/developers/agents/register-agent) documents card
registration entirely as CLI steps: `t3n agent create-card`, `t3n agent host-card`, and a `curl` to
verify. The SDK exposes the same surface programmatically:

```typescript
// index.d.ts:5227,5240,5247 — on SessionOrgDataClient
agentCardSet(input: AgentCardSetInput): Promise<MutationResponse>;
agentCardPublish(input: AgentCardRefInput): Promise<MutationResponse>;
agentCardGet(input: AgentCardRefInput): Promise<AgentCardResponse>;
```

The CLI path produces a hand-maintained `agent-card.json` that drifts from the code. The programmatic
path lets the card be generated from the same constants as the grant, so an advertised skill cannot
outlive the permission that backs it.

**Fix:** show the SDK calls alongside the CLI, at least for the "card is generated by your project"
case.

**What I did:** `src/register-card.ts` builds the card from the same `CONTRACT_FUNCTIONS` /
`ALLOWED_HOSTS` constants the grant uses, publishes it, and reads it back to confirm.

---

## 11. The claim page cannot give an agent its own identity, which is what the docs send you there for

**Severity: high** (verified against the live cluster; the documented instruction does not work)

**Confirmed empirically.** I claimed a second key exactly as instructed (same browser, same Google
account, second visit) and got a genuinely different key. It resolved to the *same* DID:

```
tenant DID : did:t3n:947e9ba8705790c014d7242cdc67624c5d9b642c
  address  : 0x718f4160d845145a7de1f52ead6dd5e08a9009e7
agent DID  : did:t3n:947e9ba8705790c014d7242cdc67624c5d9b642c
  address  : 0x1693fde558fbb3bdff8410d86420f661e8e942cd
```

Two distinct keypairs, two distinct Ethereum addresses, one identity. That is consistent with the
platform's own model: `common-errors` documents `eth_authenticator_limit` as "exceeded wallet limit
per DID (e.g. attempting 11th wallet)", so a DID is *designed* to hold many wallets. But it means a
fresh key from the claim page is a new **authenticator**, not a new **principal**.

Agent Auth asks for the latter:

> "An agent needs its **own** DID and its **own** test credits — separate from yours, and from the
> same claim page you used in Step 1"

Those two sentences cannot both hold. The claim page keys off the signed-in account, so every visit
binds another wallet to the same DID. Getting a second principal requires a different account
entirely, or the org-agent path (`createOrganisation` → `createAgent`, which mints an agent DID and
returns an opaque API key), a different authentication model that the public-agent walkthrough never
mentions.

The consequence is quiet and total: the grant is written, accepted, and enforced, but grantor and
grantee are the same DID, so the delegation demonstrates nothing. Everything looks correct.

**Reproduction:** claim a key, claim a second one from the same account, authenticate with each, and
compare the DIDs.

**Fix:** say on the claim page that a repeat visit adds a wallet to your existing DID, and state in
Agent Auth how to actually obtain a separate agent principal: a second account, or the org-agent
flow, whichever is intended.

**What I did:** `npm run whoami` authenticates both keys and exits non-zero when the DIDs match,
printing both addresses so the cause is visible. That check is the only reason I caught this before
deploying rather than after.

Then I took the only route the public-agent flow leaves open and claimed the agent's key from a
**second claim-page account**. That produced a genuinely separate principal, and it is the
configuration this repository runs on:

```
user  : did:t3n:947e9ba8705790c014d7242cdc67624c5d9b642c   signs the grant
agent : did:t3n:d7a47645b122ce1151f1f6ecdd40a4ab2de7318d   acts under it
```

Worth stating plainly, because it is the practical answer to the question this issue raises: today,
demonstrating delegation on T3N requires two claim-page accounts. Whether that is intended or whether
the org-agent flow is the supported answer is a decision only your team can make, but one of the two
should be written down where an agent builder will find it.

---

## 11b. Where the answer to "which claim page visit gives what" actually lives

**Severity: medium** (documentation routing)

Every agent needs a second identity with its own credits.
[Agent Auth](https://docs.terminal3.io/developers/adk/overview/agent-auth-adk) says so and points at
the claim page:

> "Get it a key from the same claim page you used for your own; it comes with credits attached."

Follow that link and the answer is not there. The
[claim page](https://docs.terminal3.io/developers/adk/get-started/prerequisites/request-test-tokens)
documents one sign-in producing one key, and warns:

> "Your key is shown **once**. Copy it somewhere safe before you navigate away — there's no way to
> view it again."

Read those two pages in the order the docs put them, and the reasonable conclusion is that a key is
issued per account and cannot be reissued, so the second identity must come from somewhere else.

The only page that describes the claim page's repeat behaviour at all is
[Register an Organization-owned Agent](https://docs.terminal3.io/developers/agents/provision-org-agent):

> "issues a fresh key together with metered test credits **every time you visit**"

That sentence is true and still misleads, for the reason measured in issue 11: a fresh *key* is not a
fresh *identity*. Read while looking for an agent principal, it reads like the solution. I followed
it, and it is why my first version of this report claimed the procedure was undocumented rather than
unworkable.

It is also on the wrong page. The public-agent path (Quickstart → Agent Auth → Register a Public
Agent) never links to the organisation section, so the one sentence describing the claim page's
behaviour lives where a public-agent builder has no reason to look.

The sandbox landing page adds a third partial view, advertising "20,000 test credits — enough for
**25 agents**" and "25 did:t3n verifiable agent identities", without saying how one developer obtains
25 identities, which, per issue 11, repeat claim-page visits do not provide.

**Reproduction:** follow Quickstart, then Agent Auth, then click through to the claim page. Nothing
on that path explains what a second visit does; the "shown once, no way to view it again" warning
suggests the opposite of what happens.

**Fix:** state it once on the claim page itself: a repeat visit issues a new wallet on your existing
DID, and here is how to obtain a separate agent principal. That single addition resolves both this
issue and issue 11.

---

## 12. SDK 5.10+ rejects the testnet trust manifest that 5.2 accepts

**Severity: critical on 5.10+** (nothing connects at all), **avoided by pinning 5.2**

`fetchTrustedManifest("testnet")` is step 3 of the Quickstart and the first network call any project
makes. On `@terminal3/t3n-sdk@5.10.0` it throws:

```
Error: Trust manifest at https://cn-api.sg.testnet.t3n.terminal3.io/api/trust-manifest is malformed.
    at fetchTrustedManifest (.../@terminal3/t3n-sdk/dist/index.esm.js:2:413646)
```

The endpoint is healthy: HTTP 200, valid JSON, signed 2026-08-27, publishing `cluster`, `version`,
`peer_ids`, `rtmr3_allowlist`, `signed_at` and `signature`. What it lacks is `rtmr1_allowlist`, which
5.10 declares mandatory:

```typescript
// index.d.ts:5807 — SignedTrustManifest
rtmr1_allowlist: string[];

// index.d.ts:665 — TrustAnchor
// "Base64-encoded 48-byte RTMR1 measurements … **Must be non-empty.**"
```

**The same manifest is accepted by 5.2.0**, which resolves it to an anchor of `expected_peer_ids`,
`rtmr3_allowlist` and `source`, never asking for RTMR1. So this is a client-side version gap rather
than a broken cluster: the published SDK moved to a stricter anchor before the testnet manifest
carried the field that anchor requires.

**Reproduction:**

```bash
npm i @terminal3/t3n-sdk@5.2.0    # fetchTrustedManifest("testnet") resolves
npm i @terminal3/t3n-sdk@5.10.0   # the same call throws "malformed"
```

Switching environment does not help on 5.10: `NODE_URLS` maps **both** `testnet` and `sandbox` to
`https://cn-api.sg.testnet.t3n.terminal3.io`, so the claim page's own `setEnvironment("sandbox")`
sample hits the identical failure.

**Impact while it lasts:** on 5.10+ the only way past step 3 is `{ unsafe_trust_server: true }`,
which skips DKG attestation entirely — the guarantee the platform exists to provide. An agent
demonstrated that way proves considerably less than one that verified the enclave it talked to.

**Aggravating detail:** the failure prints the SDK's obfuscated bundle to the terminal, 1.6 MB of
minified source ahead of the one line that matters. The message is recoverable only with something
like `awk 'length < 200'`.

**Fix:** publish `rtmr1_allowlist` on the testnet manifest, or have `fetchTrustedManifest` degrade
explicitly ("this cluster predates RTMR1; RTMR3-only verification will be used") instead of rejecting
the manifest as malformed. Naming the supported SDK version on the Quickstart would also save the
round trip: npm currently serves 5.15 to anyone who types `npm install @terminal3/t3n-sdk`.

**What I did:** pinned the dependency to an exact `5.2.0`, the version Terminal 3 recommends, and
this deployment runs on it with attestation verified for real and no escape hatch in the default
path. `src/session.ts` still handles 5.10+ for anyone who lands there: it attempts the real manifest
first, and when the rejection is this one it names the cause, the version gap and the
`T3N_UNSAFE_TRUST=1` opt-out instead of leaving the reader with the word "malformed".

---

## 13. Nothing says what makes a call "delegated", and the field that decides it is named after PII

**Severity: high** (a correct grant still fails, and the error points at the grant)

[Outbound HTTP is authorized by the user, not the contract](https://docs.terminal3.io/developers/adk/tips/outbound-http-auth-by-user)
states the rule precisely:

> **Delegated calls**: Use the subject user's grant
> **Direct/self calls**: Use the caller's own self-grant

What no page states is **how the node decides which kind of call it is looking at**. The answer is the
`pii_did` field on the `execute` payload: present, and the call is delegated and evaluated against
that user's grant; absent, and it is a self-call evaluated against the caller's own.

Nothing in the ADK docs connects that field to authorisation at all. Its only description in the SDK
is about audit, "the user whose data the call touched (host-stamped `pii_did`)", and the delegated
/ self distinction appears in a comment about `AuditEvent`, not in anything a caller reads while
writing an `execute`. The name completes the trap: `pii_did` reads as "set this when you are sending
personal data", so the natural implementation sets it on exactly the one function that carries PII,
which is what I did.

**The symptom is actively misleading.** Every lookup fails with:

```
host/http.egress_denied: host 'api.gleif.org' not in the authorised_hosts allowlist
```

The host *is* in the allowlist. The grant naming it was signed seconds earlier and accepted. What
failed is that the agent, being a separate DID, was checked against a self-grant it never had, so
the message describes a missing allowlist entry while the real problem is a missing delegation
marker.

**Why it hides until the last moment:** while the agent and the tenant share a DID, which is what
the claim page hands you (issue 11), the agent's self-grant *is* the user's grant, so the omission
has no effect. Everything works. The failure appears only after you fix the identity model, at which
point it looks like the fix broke a working system.

**Reproduction:** claim two keys from two accounts, grant the agent a host, and call a contract
function that makes an outbound request **without** `pii_did`. It fails with `egress_denied` for a
host that is present in the grant. Add `pii_did: <tenantDid>` and the identical call succeeds.

**Fix:** say it on the outbound-HTTP page, one line under the rule: a call is delegated when the
`execute` payload carries `pii_did`. Consider an alias (`on_behalf_of`) so the field's name matches
what it actually controls.

**What I did:** `src/agent.ts` sends `pii_did` on every call the agent makes, not just the one that
touches PII, with a comment explaining that it selects the authorisation model. The verified run
below is with two genuinely separate identities.

Terminal 3 publishes a skill file for AI coding assistants
([Using AI Coding Assistants](https://docs.terminal3.io/developers/adk/support/ai-coding-assistants))
with a twelve-row troubleshooting table. Before submitting this list I went through that table row by
row, because a bug report that repeats what you already document is noise.

**None of the eleven issues above appears in it.** The table covers runtime symptoms a developer hits
while following the docs correctly: `type: module`, unexported keys, out-of-scope variables, ACL
defaults, hex-encoding the tenant id, egress grants. Everything in this report is a different
category: places where the documentation itself is wrong, absent, or contradicts another page.

Two of the rows are worth pairing with findings above, because they show the docs disagreeing with
themselves:

| Your skill file says | But a docs page does the opposite |
|---|---|
| `tenant not found` → "Always read `tenantDid`/`did.value` from the authenticated session, never construct it" | Agent Auth assigns `await agentClient.authenticate(...)` — the `Did` object, not `.value` — straight into the grant (issue 1) |
| "Re-registering a contract breaks old pinned-version calls — re-verify any version-pinned calls after any re-registration" | Nothing warns that pinning `versionReq` in an agent grant makes every redeploy silently revoke the agent, which is the same hazard one layer up (this repo's `grant.ts` deliberately omits it) |

The skill file is good, and it is the single most useful page in the ADK docs for anyone starting
out. It is also doing work the reference pages should not be delegating to it: a developer who never
finds `/support/ai-coding-assistants` gets none of that guidance. Several rows in that table would
prevent more damage inline, next to the code they are about.

---

## Things that worked exactly as documented

Worth saying, since a bug list on its own is a distorted picture:

- `wasm32-wasip2` + `crate-type = ["cdylib", "lib"]` builds a valid component with no
  `cargo-component`, as the build page claims. `wasm-tools component wit` shows the expected
  imports and `export z:tenant-kyb/contracts@0.1.0`.
- The `readers` footgun on `maps.create` is called out in both the docs and the SDK's own JSDoc,
  which is the right amount of warning for something that fails silently.
- The `common-errors` page matched the real error strings I hit, including
  `"version <x> is not higher than current version <y>"`.
- `http-with-placeholders` error variants (`EgressDenied`, `PlaceholderDenied`,
  `PlaceholderUnknown`, `PlaceholderNoUserContext`, `UpstreamError`) map one-to-one to the WIT, so
  writing a useful error handler was mechanical.
