# Bugs and documentation issues found while building this

Everything below was hit while building `t3n-kyb-agent` against the refreshed ADK docs
(SDK `@terminal3/t3n-sdk@5.10.0`, docs as of 2026-09-06, Windows 11 / Node 24.14 / Rust 1.98).
Each entry has the reproduction, what actually happens, and the workaround that unblocked me.

Ordered by how much time they cost.

---

## 1. `agent-auth-adk` uses the DID object where a string is required — copy-paste produces a broken grant

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
`did:t3n:...` string. Two neighbouring pages get this right —
[Quickstart](https://docs.terminal3.io/developers/adk/get-started/quickstart) does
`const tenantDid = did.value;` and
[Invoke your TEE contract](https://docs.terminal3.io/developers/adk/get-started/walkthrough/invoke-contract)
does `const agentDid = agentAuth.value;` — so the Agent Auth page is the odd one out.

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

**Fix:** either mention the target override on the testing page, or drop `[build] target` from the
reference repo in favour of passing `--target wasm32-wasip2` in the documented build command.

**What I did:** the test script pins the host triple explicitly —
`cargo test --manifest-path contract/Cargo.toml --target x86_64-pc-windows-msvc`.

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
documentation page — and a literal `"latest"` cannot be used, because the server parses the field as
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

**Fix:** publish the host WIT packages somewhere versioned and link it from the WIT page — or say
plainly on the write-contract page that `wit/deps/` must be copied from `z-tenant-flight`.

**What I did:** copied `wit/deps/` from the reference repo. It is vendored in this repository so a
clone builds without a second checkout.

---

## 5. `T3N_API_KEY` is not an API key — it is a secp256k1 private key

**Severity: medium** (a naming problem with a security consequence)

The Quickstart says "Get your API key" and "Your key is shown once", then has you export it as
`T3N_API_KEY`. [Register a Public Agent](https://docs.terminal3.io/developers/agents/register-agent)
is where it becomes explicit:

```bash
export T3N_API_KEY="0x<private_key>"
```

It is a signing key: `metamask_sign(address, undefined, T3N_API_KEY)` signs the SIWE challenge with
it. Calling it an "API key" invites the handling an API key usually gets — pasted into a shared
`.env`, a CI variable, a Slack message, a support ticket. Anyone holding it can authenticate as
that identity outright.

**Fix:** name it `T3N_PRIVATE_KEY` (keeping the old name as an alias), and say on the claim page
that it is a private key, not a bearer token.

**Related, same page:** the claim step gives you one key, but every agent needs its own key with its
own credits. That only becomes clear several pages later, at `register-agent` step 2. Saying it on
the claim page would save a round trip — and an `InsufficientCreditError` that reads like a billing
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

The destination page is good — it has Enterprise, **B2B Procurement**, Payroll and Individual
sections — which makes the stub more of a shame. Inlining even a short summary with a link would
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
