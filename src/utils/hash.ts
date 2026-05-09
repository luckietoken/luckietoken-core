/**
 * SHA-256 Snapshot Integrity Hasher
 *
 * Ensures snapshot data hasn't been modified after extraction.
 * Produces a deterministic hash that can be verified independently.
 *
 * Usage:
 *   import { hashSnapshot } from "../utils/hash.ts";
 *   const digest = await hashSnapshot(snapshotJson);
 */

import { PublicKey } from "@solana/web3.js";

const encoder = new TextEncoder();

/**
 * Hash a JSON-serializable snapshot object with SHA-256.
 * Input is deterministically serialized (sorted keys) before hashing.
 *
 * @param data - The snapshot object (must be JSON-serializable)
 * @returns Hex-encoded SHA-256 digest (64 chars)
 */
export async function hashSnapshot(data: unknown): Promise<string> {
  const json = JSON.stringify(data, sortedReplacer);
  const buffer = encoder.encode(json);
  const hashBuffer = await crypto.subtle.digest("SHA-256", buffer);
  return bufferToHex(new Uint8Array(hashBuffer));
}

/**
 * Hash a raw string with SHA-256.
 *
 * @param input - Raw string to hash
 * @returns Hex-encoded SHA-256 digest (64 chars)
 */
export async function hashString(input: string): Promise<string> {
  const buffer = encoder.encode(input);
  const hashBuffer = await crypto.subtle.digest("SHA-256", buffer);
  return bufferToHex(new Uint8Array(hashBuffer));
}

/**
 * Verify that a snapshot matches an expected hash.
 *
 * @param data - The snapshot object to verify
 * @param expectedHash - The previously computed hex hash
 * @returns true if the hash matches
 */
export async function verifySnapshot(
  data: unknown,
  expectedHash: string,
): Promise<boolean> {
  const actual = await hashSnapshot(data);
  return actual === expectedHash;
}

/**
 * Decode a base58 Solana address to raw 32-byte public key.
 * Uses @solana/web3.js PublicKey for decoding.
 */
export function decodeAddress(address: string): Uint8Array {
  return new PublicKey(address).toBytes();
}

// ── Helpers ────────────────────────────────────────────────────

function sortedReplacer(_key: string, value: unknown): unknown {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return Object.keys(value)
      .sort()
      .reduce(
        (sorted: Record<string, unknown>, k: string) => {
          sorted[k] = (value as Record<string, unknown>)[k];
          return sorted;
        },
        {},
      );
  }
  return value;
}

function bufferToHex(buffer: Uint8Array): string {
  return Array.from(buffer)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
