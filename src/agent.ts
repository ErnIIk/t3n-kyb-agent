/**
 * The KYB agent.
 *
 * Authenticates as itself, runs a supplier through the enclave contract, prints
 * the verdict, and — when the verdict clears and `--submit` is passed — files
 * the onboarding record.
 *
 * Run with:
 *   npm run kyb -- --name "Deutsche Bank Aktiengesellschaft"
 *   npm run kyb -- --name "Acme GmbH" --country DE --vat 811907980 --submit
 */
import {
  CONTRACT_TAIL,
  canonicalName,
  openAgentSession,
  requireTenantDid,
  resolveContractVersion,
} from "./session.js";

interface Check {
  name: string;
  status: string;
  detail: string;
}

interface KybVerdict {
  verdict: "pass" | "review" | "fail";
  risk_score: number;
  checks: Check[];
  entity: {
    found: boolean;
    lei: string;
    legal_name: string;
    jurisdiction: string;
    entity_status: string;
    registration_status: string;
    address: string;
    candidates: string[];
  };
  vat: { valid: boolean; country_code: string; vat_number: string } | null;
}

interface OnboardingResp {
  submitted: boolean;
  status_code: number;
  reference: string;
  endpoint_host: string;
}

interface Args {
  name: string;
  lei?: string;
  country?: string;
  vat?: string;
  submit: boolean;
}

/** Minimal flag parser — a dependency here would outlive its usefulness. */
function parseArgs(argv: string[]): Args {
  const flags = new Map<string, string>();
  let submit = false;
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === "--submit") {
      submit = true;
    } else if (token.startsWith("--")) {
      flags.set(token.slice(2), argv[i + 1] ?? "");
      i += 1;
    }
  }
  const name = flags.get("name") ?? "";
  if (!name) {
    throw new Error(
      'missing --name.\n  usage: npm run kyb -- --name "Acme GmbH" [--lei <LEI>] [--country DE --vat 811907980] [--submit]',
    );
  }
  const args: Args = { name, submit };
  const lei = flags.get("lei");
  const country = flags.get("country");
  const vat = flags.get("vat");
  if (lei) args.lei = lei;
  if (country) args.country = country;
  if (vat) args.vat = vat;
  return args;
}

const STATUS_MARK: Record<string, string> = {
  pass: "PASS",
  warn: "WARN",
  fail: "FAIL",
  skipped: "SKIP",
};

function printVerdict(verdict: KybVerdict): void {
  console.log(`\nverdict : ${verdict.verdict.toUpperCase()} (risk ${verdict.risk_score}/100)`);
  if (verdict.entity.found) {
    console.log(`entity  : ${verdict.entity.legal_name} — LEI ${verdict.entity.lei}`);
    console.log(`address : ${verdict.entity.address}`);
  }
  console.log("checks  :");
  for (const check of verdict.checks) {
    console.log(`  [${STATUS_MARK[check.status] ?? check.status}] ${check.name} — ${check.detail}`);
  }
  if (verdict.entity.candidates.length > 0) {
    console.log(`other matches: ${verdict.entity.candidates.join("; ")}`);
  }
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const tenantDid = requireTenantDid();
  const scriptName = canonicalName(tenantDid, CONTRACT_TAIL);

  const agent = await openAgentSession();
  // Resolved once and reused: the server parses contract_version as SemVer, so
  // it cannot be omitted and cannot be the string "latest".
  const scriptVersion = await resolveContractVersion(scriptName);
  console.log(`agent   : ${agent.did}`);
  console.log(`contract: ${scriptName}@${scriptVersion}`);
  console.log(`supplier: ${args.name}`);

  const verdict = await agent.client.executeAndDecode<KybVerdict>({
    contract_id: scriptName,
    contract_version: scriptVersion,
    function_name: "run-kyb-check",
    input: {
      legal_name: args.name,
      ...(args.lei ? { lei: args.lei } : {}),
      ...(args.country ? { country_code: args.country } : {}),
      ...(args.vat ? { vat_number: args.vat } : {}),
    },
  });

  printVerdict(verdict);

  if (!args.submit) {
    console.log("\n(pass --submit to file the onboarding record)");
    return;
  }

  // Refusing to submit a failed supplier is the agent's job, not the
  // contract's: the contract answers questions, the agent applies policy.
  if (verdict.verdict === "fail") {
    console.log("\nnot submitting: KYB verdict is FAIL");
    process.exitCode = 1;
    return;
  }

  // pii_did names the user this agent is acting for — the subject whose profile
  // the host reads when it resolves the {{profile.*}} markers. Only this
  // function needs it; the lookups above touch no personal data.
  const submission = await agent.client.executeAndDecode<OnboardingResp>({
    contract_id: scriptName,
    contract_version: scriptVersion,
    function_name: "submit-onboarding",
    pii_did: tenantDid,
    input: {
      legal_name: args.name,
      ...(verdict.entity.lei ? { lei: verdict.entity.lei } : {}),
      ...(args.vat ? { vat_number: args.vat } : {}),
      verdict: verdict.verdict,
      risk_score: verdict.risk_score,
    },
  });

  console.log(
    `\nonboarding submitted to ${submission.endpoint_host}: HTTP ${submission.status_code}` +
      (submission.reference ? ` (ref ${submission.reference})` : ""),
  );
  console.log("the contact person's name and email were resolved inside the enclave —");
  console.log("this process never held them.");
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`\nkyb run failed: ${message}`);
  if (/egress_denied/i.test(message)) {
    console.error("  -> the host is not in the user's grant. Re-run: npm run grant");
  }
  if (/InsufficientCredit/i.test(message)) {
    console.error("  -> the AGENT identity has no credits. Claim a second key for it.");
  }
  if (/no user context|PlaceholderNoUserContext/i.test(message)) {
    console.error(
      "  -> the host could not resolve whose profile to read. Check that T3N_TENANT_DID is the\n" +
        "     user who signed the grant (npm run grant), not a second account.",
    );
  }
  process.exitCode = 1;
});
