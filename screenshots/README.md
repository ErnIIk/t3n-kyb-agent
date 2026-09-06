# Screenshots

A recorded run of the agent against T3N testnet, in the order the commands are meant to be used.
[`docs/WALKTHROUGH.md`](../docs/WALKTHROUGH.md) maps each one to the file that performs it.

| File | Command | What it shows |
|---|---|---|
| `01-quickstart.png` | `npm run quickstart` | Authenticating against the cluster: `Connected as: did:t3n:...` and `TenantClient ready.` |
| `02-whoami.png` | `npm run whoami` | Tenant and agent resolving to two different DIDs, which is what makes the delegation real |
| `03-deploy.png` | `npm run deploy` | The contract registered with its id, the `secrets` and `config` maps created, config seeded |
| `04-grant.png` | `npm run grant` | The user signing the agent's authorisation: four functions, three allowed hosts |
| `05-register-card.png` | `npm run register-card` | The public agent card published and read back, with its world-readable URL |
| `06-kyb-pass.png` | `npm run kyb -- ...` | A scored KYB verdict against live GLEIF and VIES data |
| `07-kyb-submit.png` | `npm run kyb -- ... --submit` | The onboarding record filed, and `placeholders: resolved` confirming the host substituted the submitter's details inside the enclave |
| `08-tests.png` | `npm run test:contract` | The suite passing with no credentials configured |

The seventh is the one worth looking at closely. It shows the PII guarantee holding without printing
anyone's personal data: the contract reports that the markers were resolved by checking what is
*absent* from the response, never by reading the values.
