/**
 * One-shot tenant deployment: register the contract, create its KV maps, seed
 * config. Safe to re-run — every step tolerates "already exists".
 *
 * Run with: npm run deploy
 */
import { readFile } from "node:fs/promises";
import { CONTRACT_TAIL, canonicalName, openTenantSession } from "./session.js";

const WASM_PATH = "contract/target/wasm32-wasip2/release/z_tenant_kyb.wasm";
const CARGO_TOML = "contract/Cargo.toml";

/**
 * Reads the version from Cargo.toml so there is exactly one place to bump it.
 *
 * The node refuses a re-register at a version that is not strictly higher than
 * the deployed one, and having the number live in two files is how that turns
 * into a confusing mid-deploy failure.
 */
async function contractVersion(): Promise<string> {
  const cargo = await readFile(CARGO_TOML, "utf8");
  const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error(`no version field found in ${CARGO_TOML}`);
  return match[1];
}

/** True when a failure just means the resource was already there. */
function isAlreadyExists(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /already exists/i.test(message);
}

async function main(): Promise<void> {
  const { tenant, did } = await openTenantSession();
  await tenant.tenant.me();
  console.log(`tenant  : ${did}`);

  const version = await contractVersion();
  const wasm = await readFile(WASM_PATH).catch(() => {
    throw new Error(`${WASM_PATH} not found — run "npm run build:contract" first`);
  });
  console.log(`contract: ${CONTRACT_TAIL}@${version} (${(wasm.length / 1024).toFixed(0)} KiB)`);

  let contractId: number;
  try {
    const result = await tenant.contracts.register({
      tail: CONTRACT_TAIL,
      version,
      wasm: new Uint8Array(wasm),
    });
    contractId = result.contract_id;
    console.log(`registered ${result.name} as contract id ${contractId}`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (/not higher than current version/i.test(message)) {
      throw new Error(
        `${message}\n  -> bump [package] version in ${CARGO_TOML}, rebuild, and re-run npm run deploy`,
      );
    }
    throw error;
  }

  // readers must be set explicitly: an omitted readers list defaults to
  // deny-all, and the contract's own secret read then fails with AccessDenied.
  for (const tail of ["secrets", "config"] as const) {
    try {
      await tenant.maps.create({
        tail,
        visibility: "private",
        writers: { only: [contractId] },
        readers: { only: [contractId] },
      });
      console.log(`created map ${canonicalName(did, tail)}`);
    } catch (error) {
      if (!isAlreadyExists(error)) throw error;
      console.log(`map ${canonicalName(did, tail)} already exists — reusing`);
    }
  }

  // The endpoint lives in KV so that re-pointing the agent at a different
  // procurement system needs no rebuild and no re-registration.
  const onboardingUrl = process.env.ONBOARDING_URL ?? "https://httpbin.org/post";
  await tenant.maps.entrySet("config", "onboarding_url", onboardingUrl);
  console.log(`config  : onboarding_url = ${onboardingUrl}`);

  console.log("\ndeploy complete. Next: npm run grant");
}

main().catch((error: unknown) => {
  console.error(`\ndeploy failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
