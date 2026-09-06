# Architecture

## The shape of it

```
   your machine                    T3N node                    confidential enclave
┌──────────────────┐        ┌─────────────────────┐        ┌────────────────────────┐
│ deploy.ts        │ ─────► │ tenant contracts    │ ─────► │  z_tenant_kyb.wasm     │
│  (tenant key)    │        │  register / maps    │        │                        │
├──────────────────┤        ├─────────────────────┤        │  verify-entity ────────┼──► api.gleif.org
│ grant.ts         │ ─────► │ tee:user/contracts  │        │  check-vat ────────────┼──► ec.europa.eu
│  (user signs)    │        │  agent-auth-update  │        │  run-kyb-check         │
├──────────────────┤        ├─────────────────────┤        │  submit-onboarding ────┼──► procurement
│ agent.ts         │ ─────► │ execute (as agent)  │        │        ▲               │       system
│  (agent key)     │        │  checks the grant   │        │        │ {{profile.*}} │
└──────────────────┘        └─────────────────────┘        │        └── resolved here
                                                            └────────────────────────┘
```

Three identities appear in that picture and they are deliberately distinct:

- **tenant** owns the namespace `z:<tid>:`, registers the contract, creates KV maps. Deploy-time
  authority only; it never calls the contract in normal operation.
- **user** is the data owner, whose profile holds the PII and whose signature creates the grant. In
  this repository the tenant key doubles as the user for demo simplicity; in a real deployment the
  user is an employee signing from their own device.
- **agent** has its own key, its own DID and its own credits. Holds no authority of its own; the
  grant is the entire source of what it can do.

## Why the contract has four functions rather than one

`run-kyb-check` is the function the agent actually calls. `verify-entity` and `check-vat` exist as
separate exports because:

1. They are independently useful: a procurement system that already knows a supplier's LEI wants
   only the VAT refresh, and paying for the full check would be waste.
2. The grant is per-function. A read-only integration can be granted `verify-entity` and
   `check-vat` while being denied `submit-onboarding`, which is the one function that touches PII
   and writes to an external system.

Splitting them also keeps each module small enough that its parsing logic is testable in isolation
against a captured fixture.

## The trust boundaries, concretely

| Boundary | Enforced by | What crossing it wrongly looks like |
|---|---|---|
| Agent → contract functions | the user's grant | the node refuses the call |
| Contract → external hosts | `allowedHosts` in the grant | `host/http.egress_denied` |
| Contract → user PII | `http-with-placeholders` + grant | `placeholder denied: {{profile.x}}` |
| Contract → tenant secrets | KV map ACL (`readers`) | `AccessDenied` on the KV read |
| Contract → anything else | the WIT import list | it does not compile |

That last row is the one that makes the rest credible. A WASI P2 component can only call what its
world imports, so `wit/world.wit` is a complete and auditable statement of this contract's reach:
five host interfaces, no filesystem, no sockets, no clock.

## Data flow of one `--submit` run

1. `agent.ts` authenticates with the agent key and gets `did:t3n:...` back from the node.
2. It calls `run-kyb-check` on `z:<tid>:kyb-contracts`.
3. Inside the enclave the contract calls GLEIF, then VIES if a VAT number was supplied. Neither
   request contains personal data.
4. `score()` turns both results into a verdict plus an itemised `checks` array.
5. The agent receives the verdict. It has never seen the raw registry payloads.
6. If the verdict is not `fail` and `--submit` was passed, the agent calls `submit-onboarding`.
7. The contract builds a body containing `{{profile.first_name}}`, `{{profile.last_name}}` and
   `{{profile.verified_contacts.email.value}}` as literal strings, and hands it to
   `http-with-placeholders`.
8. The host checks the grant, resolves the markers from the user's profile *inside the enclave*,
   and sends the request.
9. The contract gets back a status code and an optional reference. The plaintext values were never
   in WASM memory, in a log line, or in the agent's process.

## Design decisions worth knowing about

**Scoring lives in the contract, not the agent.** An agent is easy to modify; a registered contract
version is not. Putting the decision table inside the enclave means the rules that cleared a
supplier are the rules that were deployed, and the `checks` array is a record of which ones fired.

**The onboarding endpoint is a KV value, not a constant.** Re-pointing at a different procurement
system is `npm run deploy` with one environment variable, not a rebuild, a version bump and a
re-registration. The host allowlist in the grant still has to agree, which is deliberate: changing
where PII goes should require the user's signature again.

**Native tests cover parsing and scoring; the WASM build covers the rest.** Host-calling code only
compiles under `cfg(target_arch = "wasm32")`, so native tests cannot reach it. CI therefore does
both, `cargo test` on the host triple *and* `cargo build` for `wasm32-wasip2`. Skipping the second
one lets a broken enclave path pass a green test run; that happened once while building this and is
why the CI job is shaped this way.

**A name search is ranked, not taken in order.** GLEIF's own ordering puts a retired
`Acme International GmbH` above an active `Acme United Europe GmbH` for the query "Acme GmbH".
Reading `data[0]` would therefore fail a live supplier on the strength of a different company's dead
registration. `rank_record` scores name agreement above liveness, and the passed-over rows are
returned as `candidates` rather than discarded, because the agent should narrow a human's work
rather than hide the alternatives from them.

**The grant list is asserted by a test.** `contract/tests/allowlist.rs` reads `src/session.ts` and
fails if a host or function used by the contract is missing from the grant. Cross-language coupling
is usually a smell, but here the alternative is a mismatch that only appears at runtime, in front
of a user, as `egress_denied`.
