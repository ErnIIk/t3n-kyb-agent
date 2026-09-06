# Screenshots

Evidence for the bounty's Quickstart / Walkthrough requirement, in the order a reviewer would
reproduce them. See [`docs/WALKTHROUGH.md`](../docs/WALKTHROUGH.md) for what each step maps to.

| # | File | Command | What it shows |
|---|---|---|---|
| 1 | `01-quickstart.png` | `npm run quickstart` | `Connected as: did:t3n:…` and `TenantClient ready.` — the documented Quickstart completed |
| 2 | `02-whoami.png` | `npm run whoami` | Tenant and agent authenticated as two separate DIDs |
| 3 | `03-deploy.png` | `npm run deploy` | Contract registered, `secrets` and `config` maps created, config seeded |
| 4 | `04-grant.png` | `npm run grant` | The user signing the agent's authorisation: 4 functions, 3 allowed hosts |
| 5 | `05-register-card.png` | `npm run register-card` | Public agent card published and read back |
| 6 | `06-kyb-pass.png` | `npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980` | A full KYB verdict against live GLEIF and VIES data |
| 7 | `07-kyb-submit.png` | `npm run kyb -- … --submit` | Onboarding filed; the echoed body shows `{{profile.*}}` resolved by the host, not by the contract |
| 8 | `08-tests.png` | `npm run test:contract` | 39 tests passing with no credentials configured |
