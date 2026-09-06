/**
 * Authorises the agent to act for the user.
 *
 * This is the step people skip, and it is the one that matters: authentication
 * only proves who the agent is. Until the data owner signs this grant, the
 * agent can call nothing and reach nowhere — outbound calls fail with
 * `host/http.egress_denied` and placeholder resolution is refused.
 *
 * The grant is scoped in three dimensions at once: which contract, which
 * functions of it, and which external hosts those functions may reach.
 *
 * Run with: npm run grant
 */
import {
  ALLOWED_HOSTS,
  CONTRACT_FUNCTIONS,
  CONTRACT_TAIL,
  canonicalName,
  openAgentSession,
  openTenantSession,
} from "./session.js";

/** The node-provided contract that stores per-user agent authorisations. */
const USER_CONTRACTS = "tee:user/contracts";

async function main(): Promise<void> {
  // The agent session exists only to read back the agent's real DID — never
  // derive it from a key or hard-code it.
  const agent = await openAgentSession();
  console.log(`agent   : ${agent.did}`);

  const { client, did: tenantDid } = await openTenantSession();
  const scriptName = canonicalName(tenantDid, CONTRACT_TAIL);
  console.log(`user    : ${tenantDid}`);
  console.log(`contract: ${scriptName}`);

  await client.execute({
    contract_id: USER_CONTRACTS,
    function_name: "agent-auth-update",
    input: {
      agents: [
        {
          agentDid: agent.did,
          scripts: [
            {
              scriptName,
              functions: [...CONTRACT_FUNCTIONS],
              allowedHosts: [...ALLOWED_HOSTS],
            },
          ],
        },
      ],
    },
  });

  console.log(`\ngranted ${CONTRACT_FUNCTIONS.length} functions on ${scriptName}`);
  console.log(`allowed hosts: ${ALLOWED_HOSTS.join(", ")}`);
  console.log("\nNext: npm run kyb -- --name \"Deutsche Bank Aktiengesellschaft\"");
}

main().catch((error: unknown) => {
  console.error(`\ngrant failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
