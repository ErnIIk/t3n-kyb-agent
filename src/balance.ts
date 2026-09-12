/**
 * Prints the credit balance of both identities.
 *
 * Credits are the one resource this agent consumes that nothing else watches.
 * An agent that runs out does not fail at deploy time; it authenticates fine
 * and then fails at the first invocation with `InsufficientCreditError`, which
 * reads like a platform fault rather than an empty wallet.
 *
 * Which wallet matters: every contract call is billed to the **agent**, not the
 * tenant. A tenant with plenty of credits and an empty agent is a deployment
 * that looks healthy and can do nothing.
 *
 * Run with: npm run balance
 */
import { BASE_UNITS_PER_TOKEN } from "@terminal3/t3n-sdk";
import { openAgentSession, openTenantSession } from "./session.js";

/** Below this, top up before the next demo rather than after it. */
const LOW_WATER_MARK_T3N = 500;

/**
 * Balances come back in base units (1 T3N = 1e6), which is unreadable at a
 * glance and invites the wrong conclusion in both directions.
 */
function toTokens(row: unknown): number | null {
  if (row === null || typeof row !== "object") return null;
  const record = row as Record<string, unknown>;
  const raw = record.available ?? record.balance ?? record.amount;
  if (raw === undefined || raw === null) return null;
  const base = Number(raw);
  return Number.isFinite(base) ? base / Number(BASE_UNITS_PER_TOKEN) : null;
}

function report(label: string, did: string, row: unknown): number | null {
  const tokens = toTokens(row);
  const shown = tokens === null ? JSON.stringify(row) : `${tokens.toFixed(2)} T3N`;
  console.log(`${label} ${did}`);
  console.log(`  credits: ${shown}`);
  return tokens;
}

async function main(): Promise<void> {
  const { client: tenantClient, did: tenantDid } = await openTenantSession();
  report("tenant", tenantDid, await tenantClient.getBalance());

  const agent = await openAgentSession();
  const agentTokens = report("agent ", agent.did, await agent.client.getBalance());

  if (agentTokens !== null && agentTokens < LOW_WATER_MARK_T3N) {
    console.warn(
      `\nThe agent is below ${LOW_WATER_MARK_T3N} T3N. Contract calls bill the agent, so it is\n` +
        "the one that strands the deployment. Claim a fresh key for it, or ask the cluster\n" +
        "operator to top this DID up.",
    );
    process.exitCode = 1;
    return;
  }

  console.log("\nContract calls bill the agent, so that is the balance to watch.");
}

main().catch((error: unknown) => {
  console.error(`\nbalance check failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
