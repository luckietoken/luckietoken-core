/**
 * Merkle Tree for Holder Snapshot Verification
 *
 * Generates a Merkle root from the sorted holder list. The root is
 * stored on-chain so winners can prove their eligibility with a
 * Merkle proof (path + siblings).
 *
 * Domain-separated SHA-256 leaves/nodes to prevent second-preimage attacks.
 * Addresses are base58-decoded to raw 32-byte pubkeys before hashing,
 * matching the on-chain Rust implementation byte-for-byte.
 *
 * Usage:
 *   import { buildMerkleTree, getProof, verifyProof } from "../utils/merkle.ts";
 *
 *   const tree = await buildMerkleTree(holders);
 *   const root = tree.root;                     // Store on-chain
 *   const proof = await getProof(tree, address); // Winner provides this
 *   const valid = await verifyProof(root, proof); // On-chain verification
 */

import { decodeAddress } from "./hash.ts";

// ── Types ──────────────────────────────────────────────────────

export interface MerkleTree {
  root: string;
  leaves: string[];                 // Hashed leaves (sorted by raw pubkey bytes)
  levels: string[][];               // All levels [leaves, ..., root]
}

export interface MerkleProof {
  leaf: string;                     // Leaf hash
  siblings: string[];               // Sibling hashes from leaf to root
  index: number;                    // Leaf position in the sorted list
}

// ── Domain separation prefixes ─────────────────────────────────

const LEAF_PREFIX = new TextEncoder().encode("luckie:leaf:");
const NODE_PREFIX = new TextEncoder().encode("luckie:node:");

/**
 * Hash a holder address as a Merkle leaf.
 * Decodes the base58 address to raw 32-byte pubkey, then:
 * SHA-256("luckie:leaf:" + pubkey_bytes)
 */
async function hashLeaf(address: string): Promise<string> {
  const pubkeyBytes = decodeAddress(address);
  const combined = new Uint8Array(LEAF_PREFIX.length + 32);
  combined.set(LEAF_PREFIX);
  combined.set(pubkeyBytes, LEAF_PREFIX.length);
  const hashBuffer = await crypto.subtle.digest("SHA-256", combined);
  return bufferToHex(new Uint8Array(hashBuffer));
}

async function hashPair(left: string, right: string): Promise<string> {
  // Sort pair for deterministic ordering
  const [a, b] = left < right ? [left, right] : [right, left];
  const aBytes = hexToBuffer(a);
  const bBytes = hexToBuffer(b);

  const combined = new Uint8Array(
    NODE_PREFIX.length + aBytes.length + bBytes.length,
  );
  combined.set(NODE_PREFIX);
  combined.set(aBytes, NODE_PREFIX.length);
  combined.set(bBytes, NODE_PREFIX.length + aBytes.length);

  const hashBuffer = await crypto.subtle.digest("SHA-256", combined);
  return bufferToHex(new Uint8Array(hashBuffer));
}

// ── Tree Construction ──────────────────────────────────────────

/**
 * Build a Merkle tree from a list of holder addresses.
 * Addresses are base58-decoded to raw bytes, sorted lexicographically
 * by raw bytes for deterministic root matching the on-chain Rust implementation.
 *
 * @param addresses - List of holder wallet addresses (base58)
 * @returns MerkleTree with root, leaves, and all levels
 */
export async function buildMerkleTree(addresses: string[]): Promise<MerkleTree> {
  if (addresses.length === 0) {
    throw new Error("Cannot build Merkle tree from empty address list");
  }

  // Decode all addresses to raw bytes for sorting
  const decoded = addresses.map((addr) => ({
    addr,
    bytes: decodeAddress(addr),
  }));

  // Sort by raw bytes (matches Rust's lexicographic ordering on [u8; 32])
  decoded.sort((a, b) => {
    for (let i = 0; i < 32; i++) {
      if (a.bytes[i] !== b.bytes[i]) return a.bytes[i] - b.bytes[i];
    }
    return 0;
  });

  // Hash all leaves (using decoded bytes)
  const leaves: string[] = [];
  for (const { addr } of decoded) {
    leaves.push(await hashLeaf(addr));
  }

  const levels: string[][] = [leaves];

  // Build tree bottom-up
  let currentLevel = leaves;
  while (currentLevel.length > 1) {
    const nextLevel: string[] = [];

    for (let i = 0; i < currentLevel.length; i += 2) {
      const left = currentLevel[i];
      const right = currentLevel[i + 1];

      if (right) {
        nextLevel.push(await hashPair(left, right));
      } else {
        // Odd number of nodes — promote the last one
        nextLevel.push(left);
      }
    }

    levels.push(nextLevel);
    currentLevel = nextLevel;
  }

  return {
    root: currentLevel[0],
    leaves,
    levels,
  };
}

// ── Proof Generation ───────────────────────────────────────────

/**
 * Generate a Merkle proof for a specific address.
 *
 * @param tree - The Merkle tree (from buildMerkleTree)
 * @param address - The holder address (base58) to prove inclusion for
 * @returns MerkleProof with siblings, or null if address not in tree
 */
export async function getProof(
  tree: MerkleTree,
  address: string,
): Promise<MerkleProof | null> {
  const leaf = await hashLeaf(address);
  let index = tree.leaves.indexOf(leaf);

  if (index === -1) return null;

  const siblings: string[] = [];

  for (const level of tree.levels) {
    if (level.length <= 1) break;

    const isLeft = index % 2 === 0;
    const siblingIndex = isLeft ? index + 1 : index - 1;

    if (siblingIndex < level.length) {
      siblings.push(level[siblingIndex]);
    }

    index = Math.floor(index / 2);
  }

  return { leaf, siblings, index: tree.leaves.indexOf(leaf) };
}

// ── Proof Verification ─────────────────────────────────────────

/**
 * Verify a Merkle proof against a stored root.
 *
 * @param root - The on-chain Merkle root
 * @param proof - The MerkleProof to verify
 * @returns true if the proof is valid
 */
export async function verifyProof(
  root: string,
  proof: MerkleProof,
): Promise<boolean> {
  let hash = proof.leaf;

  for (const sibling of proof.siblings) {
    hash = await hashPair(hash, sibling);
  }

  return hash === root;
}

// ── Helpers ────────────────────────────────────────────────────

function bufferToHex(buffer: Uint8Array): string {
  return Array.from(buffer)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function hexToBuffer(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}
