/**
 * Prints both identities and stops. Run this first.
 *
 * Almost every first-run problem is one of three things: a key that was never
 * claimed, the same key used for both identities, or a missing tenant DID. All
 * three are visible here in five seconds, before anything is deployed.
 *
 * Run with: npm run whoami
 */
import { T3N_ENV, openAgentSession, openTenantSession } from "./session.js";

async function main(): Promise<void> {
  console.log(`environment: ${T3N_ENV}\n`);

  const { tenant, did: tenantDid, address: tenantAddress } = await openTenantSession();
  await tenant.tenant.me();
  console.log(`tenant DID : ${tenantDid}`);
  console.log(`  address  : ${tenantAddress}`);

  const agent = await openAgentSession();
  console.log(`agent DID  : ${agent.did}`);
  console.log(`  address  : ${agent.address}`);

  if (tenantDid === agent.did) {
    console.error(
      "\nWARNING: the tenant and the agent are the same identity.\n" +
        "  T3N_API_KEY and T3N_AGENT_KEY must be two different claimed keys — otherwise\n" +
        "  the agent is authorising itself and the delegation model proves nothing.",
    );
    process.exitCode = 1;
    return;
  }

  console.log(`\nboth identities are live. Put this in .env:\nT3N_TENANT_DID=${tenantDid}`);
}

main().catch((error: unknown) => {
  console.error(`\nwhoami failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
