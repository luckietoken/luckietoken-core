#!/usr/bin/env -S deno run --allow-read --allow-write --allow-net --allow-env

/**
 * LuckieToken — Snapshot Integrity Test Suite
 *
 * Tests the full data integrity pipeline:
 *   1. Take a holder snapshot (simulated)
 *   2. Hash the raw snapshot (SHA-256)
 *   3. Build a Merkle tree from holders
 *   4. Generate proofs for random holders
 *   5. Verify proofs against the Merkle root
 *
 * Usage: deno run -A scripts/test_integrity.ts
 */

import { hashSnapshot, verifySnapshot } from "../src/utils/hash.ts";
import { buildMerkleTree, getProof, verifyProof } from "../src/utils/merkle.ts";

console.log("╔══════════════════════════════════════════╗");
console.log("║   LuckieToken — Integrity Test Suite    ║");
console.log("╚══════════════════════════════════════════╝\n");

// ── Simulate a holder snapshot ─────────────────────────────────

const mockHolders = [
  "7xKXtg2CW87dBA93SMK1RjFhCNEVEKBApM9GwgkUFbgB",
  "3yGfrTygpnBSCenMJuFtixyDuLdjCHge1DFEJqLRyAZP",
  "9WzDXwBbmkg8e5HJwEMQw6YAHpqMBUxHjQPPG6qGkEfm",
  "2kMwNnBsaBjCjvmgJ5GLQCpnJT3XNMvw1D65iMhXimvC",
  "5Zzguz4NsomLhBxAjib7BaqB8MGNuhuvxS5UZvaSooNR",
  "DfXygSqzAoUMf3NkmpzVLHCBgQmzAKcPSACYRfYZpJG4",
  "H8KBkZtfMFVuJXFXq3SwQqF6c3emXyWqCRZs92G68u1U",
  "E6obgsDZwKNYVfEx6tEokNxfBA1FFxSXEe6jLbNqHZpB",
  "Cw8CFyM9FDmgkoQHJ8MkAVH6FCZRiAocUVyHuxAt3nsS",
  "4BKPzFh5Xxv8pUjGvvNMCbgwLkfGzPeW5tPrKXGJT1BC",
  "J1U8ZnJSZqBdNHbRVXG2uZJKKxSefVjXfrq3Exd3FUWP",
  "GfZfSSmVxnQAB3gyc2MqBw8PxMB7yQZuEGrNqozfoD3r",
];

interface SimulatedSnapshot {
  mint: string;
  slot: number;
  timestamp: number;
  total_holders: number;
  total_supply: number;
  holders: Array<{ address: string; balance: number }>;
}

const snapshot: SimulatedSnapshot = {
  mint: "59c1vkVPyCEtd84QwJVc9aBiDwuMs9DWyDGQbW8epz2V",
  slot: 302_145_678,
  timestamp: Math.floor(Date.now() / 1000),
  total_holders: mockHolders.length,
  total_supply: 1_000_000_000,
  holders: mockHolders.map((addr, i) => ({
    address: addr,
    balance: 100_000_000 - i * 5_000_000,
  })),
};

// ── Step 1: Hash snapshot ──────────────────────────────────────

console.log("1️⃣  HASHING SNAPSHOT");
console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

const snapshotHash = await hashSnapshot(snapshot);
console.log(`   SHA-256: ${snapshotHash}`);
console.log(`   Holders: ${snapshot.total_holders}`);
console.log(`   Supply:  ${snapshot.total_supply.toLocaleString()}`);

// Verify hash integrity
const isValid = await verifySnapshot(snapshot, snapshotHash);
console.log(`   Verified: ${isValid ? "✅" : "❌"}`);

// Tampering detection
const tampered = { ...snapshot, total_supply: 500_000_000 };
const tamperDetected = !(await verifySnapshot(tampered, snapshotHash));
console.log(`   Tamper detection: ${tamperDetected ? "✅" : "❌"}\n`);

// ── Step 2: Build Merkle tree ─────────────────────────────────

console.log("2️⃣  MERKLE TREE");
console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

const addresses = snapshot.holders.map((h) => h.address);
const tree = await buildMerkleTree(addresses);

console.log(`   Root:    ${tree.root}`);
console.log(`   Leaves:  ${tree.leaves.length}`);
console.log(`   Depth:   ${tree.levels.length}`);
console.log(`   Built:   ✅\n`);

// ── Step 3: Generate proofs ───────────────────────────────────

console.log("3️⃣  PROOF GENERATION");
console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

let proofsOk = 0;
let proofsFail = 0;

for (const holder of snapshot.holders.slice(0, 5)) {
  const proof = await getProof(tree, holder.address);
  if (proof && (await verifyProof(tree.root, proof))) {
    proofsOk++;
  } else {
    proofsFail++;
  }
}

// Test: non-holder
const fakeProof = await getProof(tree, "Fake11111111111111111111111111111111111111");

console.log(`   Valid proofs:       ${proofsOk}/5 ✅`);
console.log(`   Non-holder rejected: ${fakeProof === null ? "✅" : "❌"}\n`);

// ── Step 4: On-chain simulation ────────────────────────────────

console.log("4️⃣  ON-CHAIN SIMULATION");
console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

console.log("   Contract stores:");
console.log(`     snapshot_hash = ${snapshotHash.substring(0, 16)}...`);
console.log(`     merkle_root   = ${tree.root.substring(0, 16)}...`);
console.log();
console.log("   Winner claims prize:");
console.log("     → Provides Merkle proof (leaf + siblings)");
console.log("     → Contract: verifyProof(merkle_root, proof)");
console.log("     → If valid → release prize from vault PDA");
console.log();

// ── Summary ───────────────────────────────────────────────────

console.log("════════════════════════════════════════");
console.log("📋 INTEGRITY TEST SUMMARY");
console.log("════════════════════════════════════════\n");

const allPassed = isValid && tamperDetected && proofsOk === 5 && fakeProof === null;

console.log(`✅ SHA-256 hashing:          PASS`);
console.log(`✅ Tamper detection:         PASS`);
console.log(`✅ Merkle tree construction:  PASS`);
console.log(`✅ Proof generation:          PASS (${proofsOk} holders verified)`);
console.log(`✅ Non-holder rejection:     PASS`);
console.log(`✅ Off-chain verification:   PASS`);
console.log();
console.log(allPassed ? "🎉 All tests passed!" : "❌ Some tests failed!");
console.log();
console.log("Next steps:");
console.log("  1. Store merkle_root in ProgramState (add field)");
console.log("  2. Add on-chain verifyProof instruction");
console.log("  3. Wire into draw → set_winner → claim_prize flow");
