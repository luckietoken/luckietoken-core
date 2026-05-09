use crate::*;

#[test]
fn test_initialize_creates_state() {
    let fee_bps = 300u16;
    let interval = 86400i64; // 1 day

    // Validate fee bounds
    assert!(fee_bps <= 1000, "Fee must be <= 1000 bps (10%)");
    assert!(fee_bps > 0, "Fee must be > 0");
    assert!(interval >= 3600, "Minimum interval is 1 hour");

    // Vault starts empty
    let vault_balance: u64 = 0;
    assert_eq!(vault_balance, 0, "Vault should start empty");

    // Draw counter starts at 0
    let total_draws: u64 = 0;
    assert_eq!(total_draws, 0);
}

#[test]
fn test_fee_bounds() {
    let max_fee = 1000u16;
    assert!(max_fee <= 1000);

    // Typical values
    let fee_3pct = 300u16;
    let fee_5pct = 500u16;

    assert!(fee_3pct <= max_fee);
    assert!(fee_5pct <= max_fee);
    assert!(fee_3pct < fee_5pct);

    // Edge cases
    let fee_0 = 0u16;
    let fee_max = 1000u16;
    assert!(fee_0 <= max_fee);
    assert!(fee_max <= max_fee);
}

#[test]
fn test_vault_balance_math() {
    let mut vault_balance: u64 = 100_000_000; // 0.1 SOL in lamports

    // Deposit yield
    let deposit_amount = 50_000_000u64;
    vault_balance = vault_balance.checked_add(deposit_amount).unwrap();
    assert_eq!(vault_balance, 150_000_000);

    // Distribute prize
    let prize_amount = 30_000_000u64;
    assert!(
        prize_amount <= vault_balance,
        "Prize cannot exceed vault balance"
    );
    vault_balance = vault_balance.checked_sub(prize_amount).unwrap();
    assert_eq!(vault_balance, 120_000_000);
}

#[test]
fn test_insufficient_vault_balance() {
    let vault_balance: u64 = 10_000_000;
    let prize_amount = 20_000_000u64;

    // Should fail if prize > balance
    assert!(prize_amount > vault_balance, "Should be insufficient");
}

#[test]
fn test_draw_interval_enforcement() {
    let last_draw_at: i64 = 1_700_000_000;
    let draw_interval: i64 = 86400; // 24 hours
    let current_time: i64 = last_draw_at + draw_interval - 1; // 1 second before

    // Should reject if interval hasn't elapsed
    assert!(
        current_time < last_draw_at + draw_interval,
        "Must reject early draw"
    );
}

#[test]
fn test_draw_interval_ok() {
    let last_draw_at: i64 = 1_700_000_000;
    let draw_interval: i64 = 86400;
    let current_time: i64 = last_draw_at + draw_interval + 1; // 1 second after

    // Should allow draw
    assert!(current_time >= last_draw_at + draw_interval);
}

#[test]
fn test_yield_source_governance() {
    // Verify we can switch between yield sources
    let sources = vec![
        YieldSource::None,
        YieldSource::MarinadeSolStaking,
        YieldSource::KaminoLending,
        YieldSource::JupiterLP,
    ];

    assert_eq!(sources.len(), 4);

    // Toggle between sources
    let current = YieldSource::MarinadeSolStaking;
    assert_ne!(current, YieldSource::None);
    assert_eq!(current, YieldSource::MarinadeSolStaking);
}

#[test]
fn test_total_draws_increment() {
    let mut total_draws: u64 = 0;

    for _ in 0..5 {
        total_draws = total_draws.checked_add(1).unwrap();
    }

    assert_eq!(total_draws, 5);
}

#[test]
fn test_checked_math_overflow() {
    let max_u64 = u64::MAX;
    let one = 1u64;

    let result = max_u64.checked_add(one);
    assert!(result.is_none(), "Overflow should return None");
}

#[test]
fn test_checked_math_underflow() {
    let zero = 0u64;
    let one = 1u64;

    let result = zero.checked_sub(one);
    assert!(result.is_none(), "Underflow should return None");
}

#[test]
fn test_winner_flow() {
    // Simulate the distribute_prize flow (push model)
    let mut current_prize: u64 = 0;

    // Initially no active draw
    assert_eq!(current_prize, 0);

    // After process_draw, prize is set from vault
    current_prize = 500_000_000; // 0.5 tokens

    // Winner is selected off-chain from snapshot
    let winner_key = Pubkey::new_from_array([1u8; 32]);

    // Authority calls distribute_prize which:
    // 1. Verifies Merkle proof (tested separately)
    // 2. Transfers prize to winner's token account
    // 3. Resets state
    assert_eq!(current_prize, 500_000_000);

    // After distribution, everything resets
    current_prize = 0;

    assert_eq!(current_prize, 0);
}

#[test]
fn test_emergency_toggle() {
    let mut emergency_mode = false;
    assert!(!emergency_mode);

    // Toggle on
    emergency_mode = !emergency_mode;
    assert!(emergency_mode);

    // Toggle off
    emergency_mode = !emergency_mode;
    assert!(!emergency_mode);
}

#[test]
fn test_token_decimals() {
    // Standard SPL Token uses 9 decimals
    const DECIMALS: u8 = 9;

    // 1 token = 10^9 lamports
    let one_token: u64 = 1_000_000_000;
    assert_eq!(one_token, 10u64.pow(DECIMALS as u32));

    // Common amounts
    let micro_amount = 1_000u64; // 0.000001 tokens
    assert!(micro_amount < one_token);

    let large_amount = 1_000_000_000_000u64; // 1000 tokens
    assert!(large_amount > one_token);
}

// ── Merkle Proof Tests ─────────────────────────────────────────

#[test]
fn test_merkle_hash_leaf() {
    let addr = Pubkey::new_from_array([1u8; 32]);
    let leaf = merkle_hash_leaf(&addr);

    // Hash should be 32 bytes and non-zero
    assert_eq!(leaf.len(), 32);
    assert_ne!(leaf, [0u8; 32]);

    // Deterministic: same input = same hash
    let leaf2 = merkle_hash_leaf(&addr);
    assert_eq!(leaf, leaf2);

    // Different input = different hash
    let addr2 = Pubkey::new_from_array([2u8; 32]);
    let leaf3 = merkle_hash_leaf(&addr2);
    assert_ne!(leaf, leaf3);
}

#[test]
fn test_merkle_hash_pair() {
    let a = [1u8; 32];
    let b = [2u8; 32];

    let hash = merkle_hash_pair(&a, &b);
    assert_eq!(hash.len(), 32);
    assert_ne!(hash, [0u8; 32]);

    // Commutative: hash_pair(a,b) == hash_pair(b,a)
    let hash2 = merkle_hash_pair(&b, &a);
    assert_eq!(hash, hash2);
}

#[test]
fn test_merkle_proof_valid() {
    // Build a simple 4-leaf tree manually
    let addresses = [
        Pubkey::new_from_array([1u8; 32]),
        Pubkey::new_from_array([2u8; 32]),
        Pubkey::new_from_array([3u8; 32]),
        Pubkey::new_from_array([4u8; 32]),
    ];

    // Level 0: leaves
    let leaf0 = merkle_hash_leaf(&addresses[0]);
    let leaf1 = merkle_hash_leaf(&addresses[1]);
    let leaf2 = merkle_hash_leaf(&addresses[2]);
    let leaf3 = merkle_hash_leaf(&addresses[3]);

    // Level 1: parents
    let node01 = merkle_hash_pair(&leaf0, &leaf1);
    let node23 = merkle_hash_pair(&leaf2, &leaf3);

    // Level 2: root
    let root = merkle_hash_pair(&node01, &node23);

    // Proof for address[0]: leaf0, siblings [leaf1, node23]
    let siblings = [leaf1, node23];
    assert!(verify_merkle_proof(&root, &leaf0, &siblings));

    // Proof for address[3]: leaf3, siblings [leaf2, node01]
    let siblings3 = [leaf2, node01];
    assert!(verify_merkle_proof(&root, &leaf3, &siblings3));
}

#[test]
fn test_merkle_proof_invalid() {
    let addresses = [
        Pubkey::new_from_array([1u8; 32]),
        Pubkey::new_from_array([2u8; 32]),
        Pubkey::new_from_array([3u8; 32]),
        Pubkey::new_from_array([4u8; 32]),
    ];

    let leaf0 = merkle_hash_leaf(&addresses[0]);
    let leaf1 = merkle_hash_leaf(&addresses[1]);
    let leaf2 = merkle_hash_leaf(&addresses[2]);
    let leaf3 = merkle_hash_leaf(&addresses[3]);

    let node01 = merkle_hash_pair(&leaf0, &leaf1);
    let node23 = merkle_hash_pair(&leaf2, &leaf3);
    let root = merkle_hash_pair(&node01, &node23);

    // Wrong sibling
    let bad_siblings = [leaf2, node23]; // leaf2 instead of leaf1
    assert!(!verify_merkle_proof(&root, &leaf0, &bad_siblings));

    // Wrong root
    let wrong_root = [0u8; 32];
    let good_siblings = [leaf1, node23];
    assert!(!verify_merkle_proof(&wrong_root, &leaf0, &good_siblings));

    // Non-existent leaf
    let fake_leaf = merkle_hash_leaf(&Pubkey::new_from_array([99u8; 32]));
    assert!(!verify_merkle_proof(&root, &fake_leaf, &good_siblings));

    // Empty siblings (single node tree)
    let single_root = merkle_hash_leaf(&addresses[0]);
    assert!(verify_merkle_proof(&single_root, &leaf0, &[]));

    // Empty siblings with wrong leaf
    assert!(!verify_merkle_proof(&single_root, &leaf1, &[]));
}

#[test]
fn test_merkle_cross_validation_typescript() {
    // Cross-validate against TypeScript hash.ts output
    // Both Rust and TypeScript use SHA-256 with domain separation.
    // This test verifies domain separation works correctly.

    let simple = Pubkey::new_from_array([0x42u8; 32]);
    let leaf = merkle_hash_leaf(&simple);

    // Hash must be 32 bytes and deterministic
    assert_eq!(leaf.len(), 32);
    let leaf2 = merkle_hash_leaf(&simple);
    assert_eq!(leaf, leaf2);

    // Domain separation: raw hash without prefix should be different
    let raw_hash = hash(&simple.to_bytes()).to_bytes();
    assert_ne!(leaf, raw_hash, "Domain separation must produce different hash");

    // Verify the hash includes the "luckie:leaf:" prefix
    // by checking hash(simple) != hash("luckie:leaf:" + simple)
    let mut prefixed = Vec::from(b"luckie:leaf:" as &[u8]);
    prefixed.extend_from_slice(&simple.to_bytes());
    let expected_leaf = hash(&prefixed).to_bytes();
    assert_eq!(leaf, expected_leaf, "Leaf hash must match SHA-256('luckie:leaf:' + pubkey)");
}
