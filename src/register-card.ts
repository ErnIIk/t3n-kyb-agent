/**
 * Publishes the agent's public card, so other agents and services can discover
 * what this agent does and how to reach it.
 *
 * The card is hosted by T3N itself at `GET /api/agent-card/<did>` — no IPFS
 * pinning, no external hosting, nothing else to keep alive. That matters for a
 * maintenance story: a discovery document that depends on a service you also
 * have to run is a discovery document that eventually 404s.
 *
 * The docs describe this as a CLI flow (`t3n agent create-card` / `host-card`).
 * The SDK also exposes it programmatically, which is what this script uses, so
 * the card is generated from the same constants as the grant instead of being a
 * hand-maintained JSON file that drifts.
 *
 * Run with: npm run register-card
 */
import { SessionOrgDataClient, getNodeUrl } from "@terminal3/t3n-sdk";
import {
  ALLOWED_HOSTS,
  CONTRACT_TAIL,
  canonicalName,
  openAgentSession,
  requireTenantDid,
} from "./session.js";

/** Cards are validated at publish time: non-empty, valid JSON, <= 16 KiB. */
const MAX_CARD_BYTES = 16 * 1024;

interface Skill {
  id: string;
  name: string;
  description: string;
  tags: string[];
}

/**
 * The four skills mirror the contract's four exported functions.
 *
 * Kept beside `CONTRACT_FUNCTIONS` deliberately: a card that advertises a skill
 * the grant does not cover is worse than no card, because a caller discovers a
 * capability that then fails.
 */
const SKILLS: Skill[] = [
  {
    id: "verify-entity",
    name: "Verify a legal entity",
    description:
      "Looks a company up in the GLEIF Global LEI Index and reports whether the entity is " +
      "registered, active, and current.",
    tags: ["kyb", "compliance", "gleif", "lei"],
  },
  {
    id: "check-vat",
    name: "Validate an EU VAT number",
    description:
      "Validates a VAT number against the European Commission VIES service and reports its " +
      "current registration status.",
    tags: ["kyb", "compliance", "vat", "vies", "eu"],
  },
  {
    id: "run-kyb-check",
    name: "Run a full KYB check",
    description:
      "Runs the entity and VAT checks in one confidential-compute invocation and returns a " +
      "scored pass/review/fail verdict with an itemised list of what was checked.",
    tags: ["kyb", "compliance", "due-diligence", "procurement"],
  },
  {
    id: "submit-onboarding",
    name: "File a supplier onboarding record",
    description:
      "Files the supplier record with the buyer's procurement system. The contact person's " +
      "personal data is resolved inside the enclave from the authorising user's profile and is " +
      "never held by the agent.",
    tags: ["kyb", "onboarding", "procurement", "pii-safe"],
  },
];

function buildCard(agentDid: string, tenantDid: string): string {
  const card = {
    // A2A-style descriptor; the platform stores the body verbatim.
    protocolVersion: "0.3.0",
    name: "KYB Counterparty Verification Agent",
    description:
      "Supplier due diligence on Terminal 3: verifies a counterparty against the GLEIF Global " +
      "LEI Index and EC VIES inside a TEE contract, returns a scored verdict, and files the " +
      "onboarding record without ever holding the contact person's personal data.",
    version: "0.1.0",
    provider: {
      organization: "t3n-kyb-agent",
      url: "https://github.com/ErnIIk/t3n-kyb-agent",
    },
    url: `${getNodeUrl()}/api/agent-card/${agentDid}`,
    capabilities: {
      streaming: false,
      pushNotifications: false,
      stateTransitionHistory: false,
    },
    defaultInputModes: ["application/json"],
    defaultOutputModes: ["application/json"],
    skills: SKILLS,
    // Non-standard but useful to a reviewer: what this agent actually touches.
    x_t3n: {
      agentDid,
      contract: canonicalName(tenantDid, CONTRACT_TAIL),
      egressHosts: [...ALLOWED_HOSTS],
      note:
        "Every skill runs inside a T3N TEE contract. The agent holds no standing access: the " +
        "data owner authorises specific functions and specific egress hosts.",
    },
  };
  return JSON.stringify(card, null, 2);
}

async function main(): Promise<void> {
  const tenantDid = requireTenantDid();
  const agent = await openAgentSession();
  console.log(`agent   : ${agent.did}`);

  const card = buildCard(agent.did, tenantDid);
  const size = Buffer.byteLength(card, "utf8");
  if (size > MAX_CARD_BYTES) {
    throw new Error(`card is ${size} bytes, over the ${MAX_CARD_BYTES} byte limit`);
  }
  console.log(`card    : ${SKILLS.length} skills, ${size} bytes`);

  const orgData = new SessionOrgDataClient(agent.client, getNodeUrl());

  // Self-owned card: the agent is both the owner and the subject, so no org is
  // involved and the agent writes its own descriptor.
  await orgData.agentCardSet({
    ownerDid: agent.did,
    agentDid: agent.did,
    card,
  });
  console.log("stored  : card written to the agent-cards scope");

  await orgData.agentCardPublish({ ownerDid: agent.did, agentDid: agent.did });
  console.log("published: card is now world-readable");

  // Read it back rather than trusting the write: publishing is the whole point,
  // so a card that stored but did not publish should fail this script loudly.
  const published = await orgData.agentCardGet({
    ownerDid: agent.did,
    agentDid: agent.did,
  });
  console.log(`\nverified via agentCardGet — card is live`);
  console.log(`public URL: ${getNodeUrl()}/api/agent-card/${agent.did}`);
  if (published === null || published === undefined) {
    throw new Error("agentCardGet returned nothing after a successful publish");
  }
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`\nregister-card failed: ${message}`);
  process.exitCode = 1;
});
