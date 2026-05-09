#!/usr/bin/env node
/**
 * LuckieToken Full Simulation — devnet
 * 
 * Flow:
 * 1. Create 5 holder keypairs + ATAs (Token-2022)
 * 2. Transfer tokens to holders (Token-2022 transfer with 3% fee)
 * 3. Fund vault from authority
 * 4. Snapshot holders → Merkle tree → set_merkle_root
 * 5. process_draw with random seed
 * 6. Pick winner off-chain → generate Merkle proof → distribute_prize
 * 7. Print winner + verify balances
 */

const {
  Connection, Keypair, PublicKey, Transaction, TransactionInstruction,
  SystemProgram, SYSVAR_RENT_PUBKEY, sendAndConfirmTransaction,
  LAMPORTS_PER_SOL
} = require('@solana/web3.js');

const {
  getOrCreateAssociatedTokenAccount, createTransferCheckedInstruction,
  TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
  unpackAccount
} = require('@solana/spl-token');

const { createHash } = require('crypto');
const fs = require('fs');

// ── Constants ──
const DEVNET_URL = 'https://api.devnet.solana.com';
const PROGRAM_ID = new PublicKey('23u6C2yNfpoxu66bAPgoiEsCSFHrMRdGQYCTYg5zsoGg');
const MINT = new PublicKey('79ELFdDfkYB6gALuPZxsmaPrU7i5P6eKW54ZoZS1yZhB');
const DECIMALS = 9;
const FEE_BPS = 300; // 3%

// PDAs
const [STATE_PDA] = PublicKey.findProgramAddressSync(
  [Buffer.from('luckie-state')], PROGRAM_ID
);
const [VAULT_AUTH] = PublicKey.findProgramAddressSync(
  [Buffer.from('luckie-vault')], PROGRAM_ID
);
const [VAULT_ATA] = PublicKey.findProgramAddressSync(
  [VAULT_AUTH.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer(), MINT.toBuffer()],
  ASSOCIATED_TOKEN_PROGRAM_ID
);

// ── IDL Instruction Discriminators (8 bytes) ──
const IX_DISCRIMINATORS = {
  initialize:       [175, 175, 109, 31, 13, 152, 155, 237],
  fund_vault:       [26, 33, 207, 242, 119, 108, 134, 73],
  set_merkle_root:  [43, 24, 91, 60, 240, 137, 28, 102],
  process_draw:     [221, 204, 87, 234, 6, 236, 86, 52],
  distribute_prize: [153, 175, 67, 111, 205, 207, 106, 15],
};

// ── Borsh helpers ──
function borshU64(n) {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(n));
  return buf;
}

function borshPubkey(pk) {
  return pk.toBuffer();
}

const TOKEN_PROGRAM_2022_ID = TOKEN_2022_PROGRAM_ID;
const ATA_PROGRAM_ID = ASSOCIATED_TOKEN_PROGRAM_ID;

// ── Merkle Tree (domain-separated, matching Rust contract) ──
const LEAF_PREFIX = Buffer.from('luckie:leaf:');
const NODE_PREFIX = Buffer.from('luckie:node:');

function hashLeaf(pubkey) {
  const h = createHash('sha256');
  h.update(LEAF_PREFIX);
  h.update(pubkey.toBuffer());
  return h.digest();
}

function hashNode(left, right) {
  // Sort hashes bytewise (matching Rust merkle_hash_pair)
  const a = Buffer.compare(left, right) <= 0 ? left : right;
  const b = Buffer.compare(left, right) <= 0 ? right : left;
  const h = createHash('sha256');
  h.update(NODE_PREFIX);
  h.update(a);
  h.update(b);
  return h.digest();
}

function buildMerkleTree(holders) {
  let leaves = holders.map(h => hashLeaf(h));
  const tree = [leaves];
  while (leaves.length > 1) {
    const next = [];
    for (let i = 0; i < leaves.length; i += 2) {
      if (i + 1 < leaves.length) {
        next.push(hashNode(leaves[i], leaves[i + 1]));
      } else {
        next.push(leaves[i]); // odd leaf promoted
      }
    }
    tree.push(next);
    leaves = next;
  }
  return { root: tree[tree.length - 1][0], tree };
}

function generateProof(tree, holderIndex) {
  const proof = [];
  let idx = holderIndex;
  for (let level = 0; level < tree.length - 1; level++) {
    const pairIdx = idx % 2 === 0 ? idx + 1 : idx - 1;
    if (pairIdx < tree[level].length) {
      proof.push(tree[level][pairIdx]);
    }
    idx = Math.floor(idx / 2);
  }
  return proof;
}

// ── Create TransactionInstruction from IDL ──
function makeInstruction(discriminator, accounts, data) {
  const keys = accounts.map(a => ({
    pubkey: a.pubkey,
    isSigner: a.isSigner || false,
    isWritable: a.isWritable || false,
  }));
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    keys,
    data: Buffer.concat([Buffer.from(discriminator), data]),
  });
}

// ── Main ──
async function main() {
  console.log('═══════════════════════════════════════════');
  console.log('  LuckieToken Simulation — Devnet');
  console.log('═══════════════════════════════════════════\n');

  const conn = new Connection(DEVNET_URL, 'confirmed');

  // Load authority wallet
  const secretKey = Uint8Array.from(
    JSON.parse(fs.readFileSync(process.env.HOME + '/.config/solana/id.json'))
  );
  const authority = Keypair.fromSecretKey(secretKey);
  console.log(`👤 Authority: ${authority.publicKey.toBase58()}`);
  console.log(`💰 Balance: ${(await conn.getBalance(authority.publicKey)) / LAMPORTS_PER_SOL} SOL\n`);

  // 1. Create 5 holders
  console.log('📋 Step 1: Creating 5 holders...');
  const holders = [];
  // Create 5 holders funded from authority wallet (no airdrop)
  const FUND_AMOUNT = 0.02 * LAMPORTS_PER_SOL; // enough for tx fees
  for (let i = 0; i < 5; i++) {
    const kp = Keypair.generate();

    // Fund from authority
    const fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: authority.publicKey,
        toPubkey: kp.publicKey,
        lamports: FUND_AMOUNT,
      })
    );
    await sendAndConfirmTransaction(conn, fundTx, [authority], { commitment: 'confirmed' });

    // Create ATA (Token-2022)
    const ata = await getOrCreateAssociatedTokenAccount(
      conn, kp, MINT, kp.publicKey, false, 'confirmed', {},
      TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
    );
    holders.push({ kp, ata: ata.address });
    console.log(`  Holder ${i + 1}: ${kp.publicKey.toBase58().slice(0, 8)}... ATA: ${ata.address.toBase58().slice(0, 8)}...`);
  }

  // 2. Transfer 1000 tokens to each holder (authority → holder ATA)
  console.log('\n📋 Step 2: Transferring 1000 LUCK to each holder...');
  const authorityAta = await getOrCreateAssociatedTokenAccount(
    conn, authority, MINT, authority.publicKey, false, 'confirmed', {},
    TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
  );

  for (let i = 0; i < holders.length; i++) {
    const tx = new Transaction().add(
      createTransferCheckedInstruction(
        authorityAta.address, MINT, holders[i].ata,
        authority.publicKey, 1000 * 10 ** DECIMALS, DECIMALS,
        [], TOKEN_2022_PROGRAM_ID
      )
    );
    await sendAndConfirmTransaction(conn, tx, [authority], { commitment: 'confirmed' });
    console.log(`  → Sent 1000 LUCK to holder ${i + 1}`);
  }

    // 0. Initialize program state (skip if already initialized)
  const stateInfo = await conn.getAccountInfo(STATE_PDA).catch(() => null);
  if (!stateInfo) {
    console.log('\n📋 Step 0: Initializing program state...');
    const initIx = makeInstruction(
      IX_DISCRIMINATORS.initialize,
      [
        { pubkey: STATE_PDA, isWritable: true },
        { pubkey: VAULT_AUTH },
        { pubkey: VAULT_ATA, isWritable: true },
        { pubkey: MINT },
        { pubkey: authority.publicKey, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId },
        { pubkey: TOKEN_PROGRAM_2022_ID },
        { pubkey: ATA_PROGRAM_ID },
        { pubkey: SYSVAR_RENT_PUBKEY },
      ],
      (() => {
        const buf = Buffer.alloc(2 + 8 + 32);
        buf.writeUInt16LE(FEE_BPS, 0);
        buf.writeBigInt64LE(BigInt(3600), 2);
        MINT.toBuffer().copy(buf, 10);
        return buf;
      })()
    );
    await sendAndConfirmTransaction(conn, new Transaction().add(initIx), [authority], { commitment: 'confirmed' });
    console.log('  ✅ Program state initialized');
  } else {
    console.log('\n📋 Step 0: State already initialized, skipping');
  }
// 3. Fund vault
  console.log('\n📋 Step 3: Funding vault with 5000 LUCK...');
  const fundVaultIx = makeInstruction(
    IX_DISCRIMINATORS.fund_vault,
    [
      { pubkey: STATE_PDA, isWritable: true },
      { pubkey: VAULT_ATA, isWritable: true },
      { pubkey: authorityAta.address, isWritable: true },
      { pubkey: MINT },
      { pubkey: authority.publicKey, isSigner: true },
      { pubkey: VAULT_AUTH },
      { pubkey: TOKEN_PROGRAM_2022_ID },
    ],
    borshU64(5000 * 10 ** DECIMALS)
  );
  await sendAndConfirmTransaction(conn, new Transaction().add(fundVaultIx), [authority], { commitment: 'confirmed' });
  console.log('  ✅ 5000 LUCK deposited into vault');

  // 4. Snapshot → Merkle → set_merkle_root
  console.log('\n📋 Step 4: Building Merkle tree & setting root...');
  const holderPubkeys = holders.map(h => h.kp.publicKey);
  const { root, tree } = buildMerkleTree(holderPubkeys);
  console.log(`  Merkle root: ${root.toString('hex')}`);

  const setRootIx = makeInstruction(
    IX_DISCRIMINATORS.set_merkle_root,
    [
      { pubkey: STATE_PDA, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true },
    ],
    root
  );
  await sendAndConfirmTransaction(conn, new Transaction().add(setRootIx), [authority], { commitment: 'confirmed' });
  console.log('  ✅ Merkle root set on-chain');

  // 5. process_draw with random seed
  console.log('\n📋 Step 5: Processing draw...');
  const randomness = createHash('sha256').update(Date.now().toString()).digest();
  const processDrawIx = makeInstruction(
    IX_DISCRIMINATORS.process_draw,
    [
      { pubkey: STATE_PDA, isWritable: true },
      { pubkey: VAULT_ATA, isWritable: true },
      { pubkey: VAULT_AUTH },
      { pubkey: VAULT_AUTH }, // vrf — any account works (unused in code)
    ],
    randomness
  );
  await sendAndConfirmTransaction(conn, new Transaction().add(processDrawIx), [authority], { commitment: 'confirmed' });
  console.log(`  Randomness seed: ${randomness.toString('hex').slice(0, 16)}...`);

  // 6. Off-chain winner selection using the same seed
  console.log('\n📋 Step 6: Selecting winner & distributing prize...');
  const seedNum = BigInt('0x' + randomness.slice(0, 8).reverse().toString('hex'));
  const winnerIdx = Number(seedNum % BigInt(holders.length));
  const winner = holders[winnerIdx];
  const proof = generateProof(tree, winnerIdx);

  console.log(`  🎯 Winner: Holder ${winnerIdx + 1} (${winner.kp.publicKey.toBase58()})`);
  console.log(`  Proof length: ${proof.length} hashes`);

  // Verify proof locally (hashNode sorts internally, matching Rust)
  let computed = hashLeaf(winner.kp.publicKey);
  for (const p of proof) {
    computed = hashNode(computed, p);
  }
  console.log(`  Local verification: ${computed.equals(root) ? '✅ PASS' : '❌ FAIL'}`);

  // 7. distribute_prize
  console.log('\n📋 Step 7: Distributing prize to winner...');
  const prizeAmount = 5000 * 10 ** DECIMALS; // full vault

  // Borsh serialize: winner_addr (pubkey) + proof vec length (u32 LE) + proof elements
  const winnerData = Buffer.alloc(32);
  winner.kp.publicKey.toBuffer().copy(winnerData);
  const proofLen = Buffer.alloc(4);
  proofLen.writeUInt32LE(proof.length);
  const proofData = Buffer.concat(
    [winnerData, proofLen, ...proof]
  );

  const distributeIx = makeInstruction(
    IX_DISCRIMINATORS.distribute_prize,
    [
      { pubkey: STATE_PDA, isWritable: true },
      { pubkey: VAULT_ATA, isWritable: true },
      { pubkey: MINT },
      { pubkey: winner.ata, isWritable: true },
      { pubkey: VAULT_AUTH },
      { pubkey: TOKEN_PROGRAM_2022_ID },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
    ],
    proofData
  );
  await sendAndConfirmTransaction(conn, new Transaction().add(distributeIx), [authority], { commitment: 'confirmed' });

  // 8. Final balances
  console.log('\n📋 Step 8: Verifying final state...');
  const winnerInfo = await conn.getTokenAccountBalance(winner.ata);
  console.log(`  🏆 Winner balance: ${winnerInfo.value.uiAmount} LUCK`);

  console.log('\n═══════════════════════════════════════════');
  console.log('  ✅ SIMULATION COMPLETE');
  console.log(`  🎯 Winner: ${winner.kp.publicKey.toBase58()}`);
  console.log(`  💰 Prize: ~${prizeAmount / 10 ** DECIMALS} LUCK`);
  console.log('═══════════════════════════════════════════');
}

main().catch(err => {
  console.error('❌ Simulation failed:', err);
  process.exit(1);
});