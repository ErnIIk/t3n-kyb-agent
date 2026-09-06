# Maintenance

Written for whoever runs this after the person who built it has moved on, including the
possibility that it is handed over to the Terminal 3 team.

## The five-minute model

There are two identities and one artifact.

- **The tenant** owns the contract and the KV maps. Its key deploys.
- **The agent** calls the contract. Its key can do nothing until the tenant signs a grant.
- **The artifact** is `contract/target/wasm32-wasip2/release/z_tenant_kyb.wasm`, registered under
  `z:<tenant-id>:kyb-contracts`.

Everything else is a script that wires those together.

## Routine tasks

### Deploy a change to the contract

```bash
# 1. bump the version — the node refuses a re-register at the same version
$EDITOR contract/Cargo.toml     # [package] version = "0.1.1"
npm run build:contract
npm run deploy
```

The grant survives a redeploy as long as the function names have not changed. If they have, re-run
`npm run grant`.

### Point onboarding at a different system

```bash
ONBOARDING_URL=https://erp.example.com/api/suppliers \
ONBOARDING_HOST=erp.example.com \
  npm run deploy && npm run grant
```

The URL is read from the `config` KV map at call time, so no rebuild is needed. The host **must**
be in the grant as well, which is why `grant` is re-run. Otherwise the first submission fails with
`host/http.egress_denied`.

### Add a new data source

Three edits, in this order:

1. `contract/src/<source>.rs`: the lookup, plus a `pub const <SOURCE>_HOST`.
2. `contract/src/kyb.rs`: how the result affects the score, with a `Check` entry explaining it.
3. `src/session.ts`: add the host to `ALLOWED_HOSTS`, and any new function to `CONTRACT_FUNCTIONS`.

If you forget step 3, `cargo test` fails with a message naming the missing host. That test exists
because this specific mistake is invisible until runtime, where it appears as `egress_denied` in
front of whoever you were demonstrating to.

### Rotate a key

Claim a new key, replace it in `.env`, re-run `npm run whoami`. Rotating the **agent** key changes
the agent's DID, so `npm run grant` must be re-run; the old DID's grant should be considered stale.
Rotating the **tenant** key changes the tenant DID, which changes the `z:<tid>:` namespace, which is
a new deployment rather than a rotation.

## What breaks on its own

| Thing | Why it changes | How you find out | Fix |
|---|---|---|---|
| GLEIF response shape | GLEIF versions its JSON:API | `parses_a_real_gleif_payload` fails | Update `parse_gleif_response` and refresh the fixture |
| VIES field naming | The GET and POST endpoints already disagree (`isValid` vs `valid`) | `parse_vies_response` returns an error rather than a silent `false` | Both spellings are already accepted; add a third if one appears |
| VIES availability | Member-state systems go down; VIES answers `MS_UNAVAILABLE` | `service_status` in the response | Retry later; the contract reports it rather than scoring it as invalid |
| SDK auth flow | The T3N SDK is pre-1.0 and moving | `npm run whoami` fails at handshake | All auth lives in `src/session.ts`; it is the only file to change |
| Contract version collision | Someone redeployed without bumping | `deploy` prints the bump instruction | Bump `contract/Cargo.toml` |

## Refreshing the test fixtures

The fixtures are real captured responses. Refresh them when an upstream shape changes:

```bash
curl -H "Accept: application/vnd.api+json" \
  "https://api.gleif.org/api/v1/lei-records?filter%5Blei%5D=529900IH9V4I3VHQVO92&page%5Bsize%5D=1" \
  -o contract/tests/fixtures/gleif_deutsche_bank.json

curl "https://ec.europa.eu/taxation_customs/vies/rest-api/ms/DE/vat/811907980" \
  -o contract/tests/fixtures/vies_valid.json

npm run test:contract
```

If a test fails after a refresh, that is the point: the parser is out of date with reality.

## Running it without any T3N credentials

Everything except the live calls works offline, which is what CI does:

```bash
npm run test:contract     # host target — 39 tests
npm run build:contract    # wasm32-wasip2 component
npm run typecheck
```

Run these from the repo root. `contract/.cargo/config.toml` pins the WASM target and is resolved from
the current directory rather than from `--manifest-path`, so running cargo from inside `contract/`
changes what you get; see BUGS.md #2.

## Cost and quota notes

- Each contract invocation spends the **agent's** credits, not the tenant's. An agent that stops
  working with `InsufficientCreditError` needs its own top-up.
- `run-kyb-check` is one invocation that makes two upstream calls, rather than two invocations. That
  is deliberate: it halves the credit cost of the common path and keeps the intermediate lookups
  inside the enclave.

## If you are taking this over

The parts most likely to need your attention, in order:

1. `src/session.ts`: the only place that touches T3N auth. An SDK change lands here.
2. `contract/src/kyb.rs`: the scoring table. Different companies have different risk appetites;
   this is the file they will want to edit.
3. `contract/src/onboarding.rs`: the shape of the record you file. Every procurement system wants
   a different JSON body.

Nothing else should need routine edits.
