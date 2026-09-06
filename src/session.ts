/**
 * Session bootstrap shared by every script in this repository.
 *
 * The T3N handshake is identical for a tenant and for an agent — the only
 * difference is which private key signs it. Keeping that in one place is what
 * makes the rest of the scripts short enough to read in one sitting, and means
 * a change in the SDK's auth flow is a one-file change here.
 */
import "dotenv/config";
import {
  T3nClient,
  TenantClient,
  createEthAuthInput,
  type Environment,
  eth_get_address,
  fetchTrustedManifest,
  getContractVersion,
  getNodeUrl,
  loadWasmComponent,
  metamask_sign,
  setEnvironment,
} from "@terminal3/t3n-sdk";

const ENVIRONMENTS = ["sandbox", "testnet", "production"] as const;

/**
 * Testnet unless T3N_ENV says otherwise.
 *
 * Validated here rather than cast: a typo like `T3N_ENV=testnnet` would
 * otherwise surface much later as an unexplained handshake failure.
 */
export const T3N_ENV: Environment = ((): Environment => {
  const value = process.env.T3N_ENV ?? "testnet";
  if ((ENVIRONMENTS as readonly string[]).includes(value)) return value as Environment;
  throw new Error(`T3N_ENV=${value} is not one of ${ENVIRONMENTS.join(", ")}`);
})();

/**
 * Reads a required environment variable, failing with an instruction rather
 * than a stack trace — first-run failures are almost always a missing key.
 */
export function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value || value.trim().length === 0) {
    throw new Error(
      `${name} is not set. Copy .env.example to .env and fill it in — ` +
        `keys come from https://docs.terminal3.io/developers/adk/get-started/prerequisites/request-test-tokens`,
    );
  }
  return value.trim();
}

export interface Session {
  /** Authenticated low-level client. */
  client: T3nClient;
  /** The DID this key authenticated as (did:t3n:...). */
  did: string;
  /** Ethereum address derived from the key, kept for logging. */
  address: string;
}

/**
 * Authenticates one identity against the node.
 *
 * `privateKey` decides who you are: the tenant key deploys contracts, the agent
 * key calls them. They are deliberately separate identities — an agent key that
 * leaks must not be able to re-register the contract it calls.
 */
export async function openSession(privateKey: string): Promise<Session> {
  setEnvironment(T3N_ENV);

  const wasmComponent = await loadWasmComponent();
  const address = eth_get_address(privateKey);

  const client = new T3nClient({
    trustAnchor: await fetchTrustedManifest(T3N_ENV),
    wasmComponent,
    handlers: {
      EthSign: metamask_sign(address, undefined, privateKey),
    },
  });

  await client.handshake();
  const auth = await client.authenticate(createEthAuthInput(address));
  // The DID is assigned by the platform — never derived from the address.
  const did = typeof auth === "string" ? auth : auth.value;

  return { client, did, address };
}

/** Authenticates the tenant (contract owner) identity. */
export async function openTenantSession(): Promise<Session & { tenant: TenantClient }> {
  const session = await openSession(requireEnv("T3N_API_KEY"));
  const tenant = new TenantClient({
    t3n: session.client,
    baseUrl: getNodeUrl(),
    tenantDid: session.did,
  });
  return { ...session, tenant };
}

/** Authenticates the agent identity, which has its own key and its own credits. */
export async function openAgentSession(): Promise<Session> {
  return openSession(requireEnv("T3N_AGENT_KEY"));
}

/** `z:<tid>:<tail>` — the canonical name of anything this tenant owns. */
export function canonicalName(tenantDid: string, tail: string): string {
  return `z:${tenantDid.replace(/^did:t3n:/, "")}:${tail}`;
}

/**
 * Resolves the currently registered version of a contract.
 *
 * `execute` payloads are deserialised strictly into `contract_id` /
 * `contract_version` / `function_name` / `input`, and the server parses the
 * version as SemVer — omitting it fails with `missing field contract_version`,
 * and a literal "latest" fails to parse. Looking it up keeps redeploys from
 * requiring an edit here, and the SDK caches the answer per contract name.
 */
export async function resolveContractVersion(contractId: string): Promise<string> {
  return getContractVersion(getNodeUrl(), contractId);
}

/** Contract tail, shared by deploy, grant and the agent. */
export const CONTRACT_TAIL = process.env.T3N_CONTRACT_TAIL ?? "kyb-contracts";

/** Every function this contract exports, in WIT (kebab-case) spelling. */
export const CONTRACT_FUNCTIONS = [
  "verify-entity",
  "check-vat",
  "run-kyb-check",
  "submit-onboarding",
] as const;

/**
 * Hosts the contract is allowed to reach.
 *
 * This list is the security boundary: the node refuses any outbound call to a
 * host that is not in the user's grant, so adding a data source means adding it
 * here and re-running `npm run grant`.
 */
export const ALLOWED_HOSTS = [
  "api.gleif.org",
  "ec.europa.eu",
  process.env.ONBOARDING_HOST ?? "httpbin.org",
] as const;
