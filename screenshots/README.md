# Screenshots

Evidence for the bounty's Quickstart / Walkthrough requirement, in the order a reviewer would
reproduce them. See [`docs/WALKTHROUGH.md`](../docs/WALKTHROUGH.md) for what each step maps to.

## Before you start

Set the workaround flag once per terminal session. The testnet trust manifest omits a field SDK 5.10
requires, so nothing connects without it; see [`BUGS.md`](../BUGS.md) #12.

PowerShell:

```powershell
cd C:\Users\ersll\source\repos\t3n-kyb-agent
$env:T3N_UNSAFE_TRUST="1"
```

bash / zsh:

```bash
cd ~/source/repos/t3n-kyb-agent
export T3N_UNSAFE_TRUST=1
```

Then run the commands below in order. Each one is one screenshot.

| # | File | Command | What it shows |
|---|---|---|---|
| 1 | `01-quickstart.png` | `npm run quickstart` | `Connected as: did:t3n:…` and `TenantClient ready.` — the documented Quickstart completed |
| 2 | `02-whoami.png` | `npm run whoami` | Tenant and agent authenticated as two **different** DIDs |
| 3 | `03-deploy.png` | `npm run deploy` | Contract registered with its id, `secrets` and `config` maps created, config seeded |
| 4 | `04-grant.png` | `npm run grant` | The user signing the agent's authorisation: 4 functions, 3 allowed hosts |
| 5 | `05-register-card.png` | `npm run register-card` | Public agent card published, read back, and its public URL |
| 6 | `06-kyb-pass.png` | `npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980` | A full KYB verdict against live GLEIF and VIES data |
| 7 | `07-kyb-submit.png` | `npm run kyb -- --name "Deutsche Bank Aktiengesellschaft" --country DE --vat 811907980 --submit` | Onboarding filed — HTTP 200, with the contact person's details resolved inside the enclave |
| 8 | `08-tests.png` | `npm run test:contract` | 40 tests passing with no credentials configured |

Optional but persuasive: the agent card served publicly by T3N, proving the agent is discoverable
by anyone, not just locally:

```powershell
curl.exe "https://cn-api.sg.testnet.t3n.terminal3.io/api/agent-card/did:t3n:<your-agent-did>"
```

## What each screenshot is evidence of

- **1** shows "complete the Quickstart" from the bounty scope, done against the live cluster.
- **2** shows the delegation model is real: two separate principals, not one identity talking to itself.
- **3–5** cover Walkthrough steps 2, 3 and the public-agent registration.
- **6–7** show the agent doing its job end to end, including the PII guarantee.
- **8** shows the build verifying without any credentials, which is what makes it maintainable by someone
  who is not the author.
