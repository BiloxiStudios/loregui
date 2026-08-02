import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const sharedRoot = dirname(fileURLToPath(import.meta.url));
const contractPath = join(sharedRoot, "artifact-contract.v1.json");

const expectedJourneyIds = [
  "NEG-STARTMENU-HOST-DOT",
  "NEG-SYSTEM32-HOST-DOT",
  "NEG-SYSTEM32-HOST-DOTDOT",
  "NEG-SYSTEM32-HOST-LORE",
  "NEG-SYSTEM32-HOST-DRIVEREL",
  "NEG-SYSTEM32-HOST-EMPTY",
  "NEG-SYSTEM32-HOST-SPACE",
  "NEG-SYSTEM32-HOST-CANCEL",
  "NEG-SYSTEM32-CREATE-LORE",
  "NEG-SYSTEM32-OPEN-DOT",
  "NEG-SYSTEM32-CLONE-DRIVEREL",
  "NEG-SYSTEM32-PALETTE-LORE",
  "NEG-SYSTEM32-LEGACY-RESTART",
  "POS-STARTMENU-HOST-SPACES",
  "POS-SYSTEM32-HOST-SPACES",
  "POS-SYSTEM32-CREATE-BACKSLASH",
  "POS-SYSTEM32-OPEN-BACKSLASH",
  "POS-SYSTEM32-CLONE-SPACES",
  "POS-SYSTEM32-JOIN",
  "POS-SYSTEM32-UNC-HOST",
  "POS-SYSTEM32-UNC-CLONE",
  "POS-SYSTEM32-CREATE-PICKER-BACKSLASH",
  "POS-SYSTEM32-OPEN-MANUAL-BACKSLASH",
  "POS-SYSTEM32-CLONE-PICKER-SPACES",
  "POS-SYSTEM32-JOIN-PICKER-LOCALROOT",
];

const expectedActorPaths = [
  "raw/run.json",
  "raw/artifact-index.json",
  "raw/mutation-ledger.jsonl",
  "raw/journeys/{journey_id}/result.json",
  "raw/journeys/{journey_id}/screenshot.png",
  "raw/journeys/{journey_id}/ui-tree.xml",
  "raw/journeys/{journey_id}/trace.jsonl",
  "raw/cleanup.json",
  "raw/terminal.json",
];

async function loadContract() {
  let bytes;
  try {
    bytes = await readFile(contractPath, "utf8");
  } catch (error) {
    assert.fail(`neutral v1 contract is missing: ${error.message}`);
  }

  try {
    return JSON.parse(bytes);
  } catch (error) {
    assert.fail(`neutral v1 contract is not valid JSON: ${error.message}`);
  }
}

test("v1 binds the reviewed candidate and real Windows sidecar", async () => {
  const contract = await loadContract();

  assert.equal(contract.schema, "sbai-5841-artifact-contract/v1");
  assert.equal(contract.contract_version, 1);
  assert.equal(contract.ticket, "SBAI-5841");
  assert.deepEqual(contract.candidate, {
    repository: "https://github.com/BiloxiStudios/loregui.git",
    pull_request: 446,
    full_sha: "49ffa08737be832eaaaab04a6a3f85dc4173b087",
    lore_pin: "9664606f5a4708606642a6670a57d16bd3d37596",
    sidecar: {
      sha256: "f4ccff72e4604db520e2dd98218f5204ef6e206150255a895a1198cb83f1f216",
      size_bytes: 36815872,
      pe_machine: "0x8664",
      target_triple: "x86_64-pc-windows-msvc",
      target_name: "loreserver-x86_64-pc-windows-msvc.exe",
    },
    prohibited_artifact_ids: ["8785575713"],
  });
});

test("v1 freezes the exact 13-negative and 12-positive journey inventory", async () => {
  const contract = await loadContract();
  const ids = contract.journey_inventory.rows.map(({ id }) => id);

  assert.equal(contract.journey_inventory.count, 25);
  assert.deepEqual(ids, expectedJourneyIds);
  assert.equal(new Set(ids).size, 25);
  assert.equal(ids.filter((id) => id.startsWith("NEG-")).length, 13);
  assert.equal(ids.filter((id) => id.startsWith("POS-")).length, 12);
  assert.deepEqual(contract.journey_inventory.set_policy, {
    exact_set_required: true,
    extra: "reject",
    missing: "reject",
    duplicate: "reject",
    zero_rows: "reject",
  });

  const coverage = contract.journey_inventory.coverage_split;
  assert.equal(coverage.packaged_routing_proof_required, true);
  assert.equal(coverage.removing_packaged_negatives_invalidates_source_only_coverage, true);
  assert.deepEqual(coverage.source_policy_rejections, [
    "RootRelative",
    "IncompleteUnc",
    "DeviceNamespace",
    "UnsupportedVerbatim",
  ]);
});

test("v1 makes the actor structurally unable to certify or mint PASS", async () => {
  const contract = await loadContract();
  const actorPaths = contract.actor_artifacts.paths.map(({ path }) => path);

  assert.deepEqual(actorPaths, expectedActorPaths);
  assert.deepEqual(contract.roles.actor.may, [
    "execute",
    "emit-raw-artifacts",
    "cleanup",
  ]);
  assert.deepEqual(contract.roles.actor.must_not, [
    "verify",
    "certify-venue",
    "certify-tools",
    "certify-package",
    "certify-firewall",
    "publish",
    "promote",
    "mint-pass",
  ]);
  assert.equal(actorPaths.some((path) => /witness|publication|pass/i.test(path)), false);
  assert.deepEqual(
    contract.actor_artifacts.paths.find(({ path }) => path === "raw/mutation-ledger.jsonl"),
    {
      path: "raw/mutation-ledger.jsonl",
      media_type: "application/x-ndjson",
      record_type: "mutation_ledger_entry",
      cardinality: "exactly-one",
      minimum_lines: 0,
    },
  );
  assert.deepEqual(contract.record_types.actor_terminal.states, [
    "execution-complete",
    "failed",
    "incomplete",
  ]);
  assert.equal(contract.record_types.actor_terminal.states.includes("PASS"), false);
  assert.equal(contract.record_types.actor_terminal.write_order, "last-actor-write");
  assert.equal(contract.record_types.actor_terminal.actor_writes_after_terminal, "forbidden");
  assert.equal(
    contract.record_types.actor_run.required_fields.candidate_sha,
    "literal:49ffa08737be832eaaaab04a6a3f85dc4173b087",
  );
  assert.equal(contract.record_types.actor_run.prewrite_validation_required, true);
});

test("v1 rejects an unapproved or ambiguous run root before mutation", async () => {
  const contract = await loadContract();

  assert.deepEqual(contract.run.root, {
    source: "caller",
    approved_base: "C:\\QA\\SBAI-5841\\runs",
    path_template: "C:\\QA\\SBAI-5841\\runs\\{run_id}",
    run_id_format: "lowercase-uuid-v4",
    must_exist: true,
    must_be_absolute: true,
    must_be_new_empty_leaf: true,
    canonical_path_must_match: true,
    reparse_points_allowed: false,
    decision_on_unknown_or_mismatch: "reject-before-actor-artifact-or-mutation",
  });
  assert.equal(contract.actor_artifacts.applies_when, "contract-pin-and-run-root-preflight-accepted");
  assert.deepEqual(contract.run.preflight.rejection_profile, {
    actor_artifacts_required: false,
    actor_artifacts_allowed: false,
    ledger_entries_allowed: false,
    mutation_allowed: false,
    witness_may_record_preflight_failure: true,
  });
});

test("v1 requires exact-byte pins and bilateral mismatch rejection", async () => {
  const contract = await loadContract();

  assert.deepEqual(contract.compatibility, {
    digest_algorithm: "sha256",
    digest_scope: "exact-file-bytes",
    pin_mode: "exact-sha256",
    unknown_schema: "reject",
    unknown_field: "reject",
    unilateral_bump: "reject",
  });
  assert.equal(contract.cross_component_conformance.contract_path, "evidence/SBAI-5841/shared/artifact-contract.v1.json");
  assert.equal(
    contract.cross_component_conformance.actor_rejects_before,
    "actor-artifact-or-ledger-entry-or-mutation",
  );
  assert.equal(
    contract.cross_component_conformance.witness_rejects_before,
    "raw-artifact-consumption-or-partial-publication-or-promotion-or-pass",
  );
  assert.deepEqual(
    contract.cross_component_conformance.mismatch_cases.map(({ id }) => id),
    ["contract-sha256-one-nibble", "producer-role-mismatch"],
  );
  assert.equal(
    contract.cross_component_conformance.mismatch_cases.every(
      ({ shared_seed, actor_validation_surface, witness_validation_surface }) =>
        shared_seed === "mutated-actor-run-envelope" &&
        actor_validation_surface === "prewrite-actor-run-envelope-validator" &&
        witness_validation_surface === "raw/run.json-consumer",
    ),
    true,
  );
});

test("v1 cross-binds index, cleanup, terminal, and PASS predicates", async () => {
  const contract = await loadContract();

  assert.deepEqual(contract.record_types.cross_record_invariants, [
    "every-actor-json-and-jsonl-record-contract-sha256-equals-sha256-of-exact-contract-bytes",
    "every-actor-json-and-jsonl-record-run-id-equals-raw-run-run-id",
    "contract-sha256-or-run-id-mismatch-is-rejected-as-cross-run-splicing",
  ]);
  assert.equal(contract.record_types.mutation_ledger_entry.required_fields.contract_sha256, "sha256");
  assert.equal(contract.record_types.mutation_ledger_entry.required_fields.run_id, "run_id");
  assert.equal(contract.record_types.trace_entry.required_fields.contract_sha256, "sha256");
  assert.equal(contract.record_types.trace_entry.required_fields.run_id, "run_id");
  assert.equal(contract.actor_artifacts.index.covers, "every-present-raw-file-except-index-and-terminal");
  assert.deepEqual(contract.record_types.artifact_index.invariants, [
    "entries-have-unique-paths-in-utf8-bytewise-ascending-order",
    "entry-set-equals-every-present-raw-path-except-index-and-terminal",
    "entry-path-media-type-size-and-sha256-match-exact-file-bytes",
    "entry-count-equals-28-plus-three-times-attempted-journey-count",
  ]);
  assert.deepEqual(contract.record_types.actor_cleanup.invariants, [
    "ledger-entry-count-equals-ledger-line-count",
    "entries-map-one-to-one-to-ledger-sequences",
    "first-ledger-entry-opens-cleanup-guaranteed-region",
    "cleanup-runs-on-every-controlled-exit-after-first-ledger-entry",
    "actor-observation-is-not-witness-certification",
  ]);
  assert.deepEqual(contract.witness_records.record_types.verification.pass_requires, [
    "actor-terminal-state-is-execution-complete",
    "exact-journey-set-is-true",
    "negative-zero-effects-is-true",
    "positive-selected-roots-is-true",
    "cleanup-restored-is-true",
    "raw-artifacts-rehashed-is-true",
    "all-required-witness-records-are-valid",
    "common-envelope-errors-is-empty",
  ]);
  assert.deepEqual(contract.witness_records.record_types.pass.allowed_only_after, [
    "verification-pass-with-every-pass-predicate-true",
    "destination-rehash-all-match",
    "atomic-promotion",
  ]);
});
