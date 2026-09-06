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
  /** Whether the host substituted the profile markers: see the contract. */
  placeholders: "resolved" | "unresolved" | "unknown";
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

  // pii_did names the user this agent is acting for. Despite the name it does
  // more than gate PII: it is what makes the call *delegated*. Outbound HTTP is
  // authorised by the subject user's grant on a delegated call, and by the
  // caller's own self-grant on a direct one — so without this, a separate agent
  // identity is checked against a self-grant it does not have, and every lookup
  // fails with `host/http.egress_denied` even though the user's grant lists the
  // host. See BUGS.md #13.
  const verdict = await agent.client.executeAndDecode<KybVerdict>({
    contract_id: scriptName,
    contract_version: scriptVersion,
    function_name: "run-kyb-check",
    pii_did: tenantDid,
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

  // Here pii_did carries its documented meaning as well: it is the subject
  // whose profile the host reads when resolving the {{profile.*}} markers.
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

  // The contract reports this by looking for markers that are absent from the
  // response, never by reading the values. Printing the state rather than the
  // data is the point: it shows the guarantee held without restating the PII.
  switch (submission.placeholders) {
    case "resolved":
      console.log("placeholders: resolved by the host inside the enclave.");
      console.log("the contact person's name and email were never held by this process.");
      break;
    case "unresolved":
      console.error("placeholders: NOT resolved. The record was filed with template strings");
      console.error("  instead of the submitter's details. Check that the calling user has a");
      console.error("  complete profile and that this agent is authorised to read those fields.");
      process.exitCode = 1;
      break;
    default:
      console.log("placeholders: endpoint did not echo the request, so resolution is unverified.");
      console.log("the contact person's details were still never held by this process.");
  }
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
