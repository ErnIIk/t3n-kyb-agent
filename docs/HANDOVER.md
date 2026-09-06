# Handover

The bounty asks whether the author wants to keep running the agent or hand it to Terminal 3, and for
the handover process either way.

**The intent is to hand this over to Terminal 3 to maintain.** This page is that process, written so
it can be executed by someone who has never spoken to the author. Until the transfer happens, the
agent stays deployed on testnet and the author keeps it running.

## What a new owner actually receives

Nothing in this project is tied to the author's machine or accounts:

- **No hosted service.** There is no server, no database, no cron host, no queue. The only deployed
  artifact is a WASM contract registered on a T3N node, and the only stored state is two tenant KV
  maps (`secrets`, `config`).
- **No proprietary credentials.** GLEIF and VIES are public and keyless. The `secrets` map exists for
  future paid sources; today it holds nothing the agent needs.
- **No author-specific configuration.** The tenant DID comes from whichever key runs `deploy`, and
  the contract's namespace follows it. The onboarding endpoint is a KV value.
- **No unreleased knowledge.** Everything learned while building this is in
  [`BUGS.md`](../BUGS.md), [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`MAINTENANCE.md`](MAINTENANCE.md).

That is the point of the handover being short: the design decisions that make it short are the same
ones the "ease of maintenance" criterion asks for.

## Handover process

Roughly 20 minutes, most of which is waiting on a Rust build.

**1. Transfer the repository.** GitHub → Settings → Transfer ownership, or fork it; nothing in the
code references the original owner except the `provider.url` field of the agent card, which
`register-card.ts` regenerates.

**2. Claim two keys** on the claim page — one tenant, one agent — and put them in `.env`.

**3. Deploy under the new tenant:**

```bash
npm install
rustup target add wasm32-wasip2
npm run quickstart     # prints the new tenant DID -> .env as T3N_TENANT_DID
npm run whoami         # confirms the two identities are distinct
npm run setup          # build + register + create maps + sign the grant
npm run register-card  # publish the agent card under the new agent DID
```

The contract is now `z:<new-tenant>:kyb-contracts`. The old deployment is untouched and can be left
to expire — the two do not interact.

**4. Verify:**

```bash
npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980
```

Expect `verdict : PASS (risk 0/100)` with five passing checks.

**5. Turn on the weekly smoke test.** Add `T3N_API_KEY`, `T3N_AGENT_KEY` and `T3N_TENANT_DID` as
repository secrets. [`.github/workflows/smoke.yml`](../.github/workflows/smoke.yml) then runs the
real agent against testnet every Monday and fails loudly if it stops working. Without the secrets it
skips and passes, so nothing breaks if you would rather not store keys.

**6. Decide who fields the failures.** GitHub emails the repository owner on a failed scheduled run.
That is the entire on-call story, deliberately.

## What the new owner is signing up for

Honest maintenance load, based on what actually moved during the build:

| Frequency | Task | Effort |
|---|---|---|
| When the SDK bumps a major | Check `src/session.ts` — all T3N auth is in that one file | ~30 min |
| When a smoke test fails | Read the run log; upstream-vs-T3N is already separated by job | ~15 min |
| When GLEIF or VIES changes a response shape | Fixture test fails; update the parser and refresh the fixture | ~1 hour |
| When someone wants a new data source | Three edits; a test fails if you miss the third | ~2 hours |
| Never | Rotating infrastructure, renewing certificates, paying for anything | — |

The realistic risk is the SDK: the docs were validated against 3.x while npm serves 5.10.0
(see BUGS.md #9), so an auth change is the most likely thing to need attention. That is why every
`execute` call and every handshake goes through `session.ts` rather than being spread across scripts.

## Before the transfer

Nothing has to happen for the agent to keep working in the meantime: it is deployed on testnet, the
weekly smoke test reports whether it still works, and CI stays green without credentials. The author
is available to walk someone through the steps above or to answer questions during the transfer.
