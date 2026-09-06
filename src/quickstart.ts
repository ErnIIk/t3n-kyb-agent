/**
 * The documented Quickstart, kept verbatim.
 *
 * This file deliberately duplicates logic that `session.ts` already wraps. It
 * exists for two reasons:
 *
 * 1. The bounty asks contributors to complete the Quickstart and Walkthrough.
 *    This is that Quickstart, following
 *    https://docs.terminal3.io/developers/adk/get-started/quickstart
 *    plus the TenantClient step from "Set Up Development Environment", so the
 *    documented flow can be run and shown on its own.
 *
 * 2. It is the smallest possible reproduction for the auth-related issues in
 *    BUGS.md. When something breaks after an SDK bump, running this first tells
 *    you whether the problem is in the platform or in this repository.
 *
 * Run with: npm run quickstart
 */
import "dotenv/config";
import {
  T3nClient,
  TenantClient,
  createEthAuthInput,
  eth_get_address,
  fetchTrustedManifest,
  getNodeUrl,
  loadWasmComponent,
  metamask_sign,
  setEnvironment,
} from "@terminal3/t3n-sdk";

setEnvironment("testnet"); // the public SDK defaults to testnet; set it explicitly anyway

const T3N_API_KEY = process.env.T3N_API_KEY;
if (!T3N_API_KEY) {
  throw new Error("T3N_API_KEY is not set — copy .env.example to .env and paste your key");
}

const wasmComponent = await loadWasmComponent(); // all crypto runs inside the WASM component
const address = eth_get_address(T3N_API_KEY);

// The documented line is exactly this:
//   trustAnchor: await fetchTrustedManifest("testnet"),
// It currently throws, because the testnet manifest omits `rtmr1_allowlist`
// and SDK 5.10 requires it (BUGS.md #12). The try/catch keeps the documented
// call as the default and only falls back when explicitly asked to.
const trustAnchor =
  process.env.T3N_UNSAFE_TRUST === "1"
    ? ({ unsafe_trust_server: true } as const)
    : await fetchTrustedManifest("testnet"); // pins the node's attestation

const t3n = new T3nClient({
  trustAnchor,
  wasmComponent,
  handlers: {
    EthSign: metamask_sign(address, undefined, T3N_API_KEY),
  },
});

await t3n.handshake();
const did = await t3n.authenticate(createEthAuthInput(address));

// Read the DID back from the authenticated session — never derive it from the
// key or hard-code it. `authenticate()` returns a `Did` object, so the `.value`
// here is required; see BUGS.md #1 for a docs page that omits it.
const tenantDid = did.value;

console.log("Connected as:", tenantDid);

// "Set Up Development Environment" — the TenantClient every later step needs.
const tenant = new TenantClient({
  t3n,
  baseUrl: getNodeUrl(),
  tenantDid,
});

await tenant.tenant.me();
console.log("TenantClient ready.");
console.log(`\nPut this in .env:\nT3N_TENANT_DID=${tenantDid}`);
