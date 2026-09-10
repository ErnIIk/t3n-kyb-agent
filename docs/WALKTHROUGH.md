# Quickstart and Walkthrough: what was completed, and where it lives

The T3 ADK documentation has a Quickstart and a five-step Walkthrough. This page maps each documented
step to the file in this repository that performs it, so a reader can follow the platform's own path
through this codebase without reading all of it.

Every step below was run against testnet, against contract id 981.

## Quickstart

| Doc step | Where it lives | Verified by |
|---|---|---|
| 1. Get your API key | `.env` (gitignored), template in `.env.example` | — |
| 2. Set up your project | `package.json`, `tsconfig.json` | `npm run typecheck` |
| 3. Connect and authenticate | [`src/quickstart.ts`](../src/quickstart.ts) | `npm run quickstart` |
| 4. Run it | `npm run quickstart` prints `Connected as: did:t3n:…` | run it yourself |
| Set Up Dev Env (TenantClient) | same file, `tenant.tenant.me()` | prints `TenantClient ready.` |

`src/quickstart.ts` follows the documented flow directly rather than reusing this project's
`session.ts` helper. That duplication is deliberate: it keeps a runnable copy of the documented path,
which is also the smallest reproduction for the auth issues in [`BUGS.md`](../BUGS.md).

## Walkthrough

| Doc step | Where it lives | Verified by |
|---|---|---|
| 1. Write your TEE contract | [`contract/src/`](../contract/src), world in [`contract/wit/world.wit`](../contract/wit/world.wit) | `cargo test` — 42 tests |
| 2. Build your TEE contract | `npm run build:contract` | `wasm-tools component wit` in CI |
| 3. Register your TEE contract | [`src/deploy.ts`](../src/deploy.ts) | `npm run deploy` prints the contract id |
| 4. Invoke your TEE contract | [`src/agent.ts`](../src/agent.ts) | `npm run kyb -- --name …` |
| 5. Test your TEE contract | `contract/src/**/tests`, [`contract/tests/allowlist.rs`](../contract/tests/allowlist.rs) | `npm run test:contract` |

Supporting pages that turned out to be required rather than optional:

| Doc page | Where it lives |
|---|---|
| Create Tenant KV Maps | `src/deploy.ts`, creates `secrets` and `config` with explicit `readers` |
| Seed API key into secrets map | `src/deploy.ts`, `maps.entrySet`, see BUGS.md #6 |
| Agent Auth | [`src/grant.ts`](../src/grant.ts), the user's signed `agent-auth-update` |
| Register a Public Agent | [`src/register-card.ts`](../src/register-card.ts), via SDK, see BUGS.md #10 |
| Placeholders in outbound calls | [`contract/src/onboarding.rs`](../contract/src/onboarding.rs) |
| Common errors | mapped to actionable hints in `src/agent.ts` and `src/deploy.ts` |

## The order a reviewer should run it in

```bash
npm install
rustup target add wasm32-wasip2
cp .env.example .env          # paste two keys: tenant and agent

npm run quickstart            # Quickstart — prints your tenant DID
# put the printed DID into .env as T3N_TENANT_DID
npm run whoami                # confirms tenant and agent are separate identities

npm run setup                 # build + register contract, create maps, sign the grant
npm run register-card         # publish the public agent card

npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980
npm run kyb -- --name "Acme GmbH" --country DE --vat 811907980 --submit
```

Steps that need no credentials at all, useful for checking the build without claiming a key:

```bash
npm run test:contract
npm run build:contract
npm run typecheck
```
