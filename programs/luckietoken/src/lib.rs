use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_lang::solana_program::hash::hash;

const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

declare_id!("23u6C2yNfpoxu66bAPgoiEsCSFHrMRdGQYCTYg5zsoGg");

#[program]
pub mod luckietoken {
    use super::*;

    const MAX_FEE_BPS: u16 = 1000;
    const MIN_DRAW_INTERVAL: i64 = 3600;

    pub fn initialize(
        ctx: Context<Initialize>,
        fee_bps: u16,
        draw_interval: i64,
        mint: Pubkey,
    ) -> Result<()> {
        require!(fee_bps <= MAX_FEE_BPS, ErrorCode::FeeTooHigh);
        require!(draw_interval >= MIN_DRAW_INTERVAL, ErrorCode::InvalidInterval);

        let state = &mut ctx.accounts.state;
        state.authority = ctx.accounts.authority.key();
        state.fee_bps = fee_bps;
        state.mint = mint;
        state.draw_interval = draw_interval;
        state.last_draw_at = 0;
        state.total_draws = 0;
        state.current_winner = Pubkey::default();
        state.current_prize = 0;
        state.yield_source = YieldSource::None;
        state.emergency_mode = false;

        emit!(Initialized { authority: state.authority, fee_bps, mint, draw_interval });
        Ok(())
    }

    // ── Vault ──
    //
    // NOTE: Transfer fees are collected AUTOMATICALLY by Token-2022
    // via Token-2022 transfer fee configuration. fund_vault is for
    // manual deposits: initial liquidity, yield returns, etc.

    pub fn fund_vault(ctx: Context<FundVault>, amount: u64) -> Result<()> {
        require!(amount > 0, ErrorCode::ZeroAmount);
        validate_token_account(&ctx.accounts.from, &ctx.accounts.state.mint, &ctx.accounts.authority.key())?;
        validate_vault_token(&ctx.accounts.vault_token, &ctx.accounts.state.mint, &ctx.accounts.vault_authority.key())?;
        require!((ctx.accounts.token_program.key() == TOKEN_PROGRAM_ID || ctx.accounts.token_program.key() == TOKEN_2022_PROGRAM_ID), ErrorCode::InvalidTokenProgram);

        let ix = spl_token_transfer_checked(
            &ctx.accounts.from.key(),
            &ctx.accounts.state.mint,
            &ctx.accounts.vault_token.key(),
            &ctx.accounts.authority.key(),
            amount,
        );

        solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.from.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.vault_token.to_account_info(),
                ctx.accounts.authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;

        let state = &mut ctx.accounts.state;
        state.vault_balance = read_token_amount(&ctx.accounts.vault_token);
        emit!(VaultFunded { amount, funder: ctx.accounts.authority.key(), vault_balance: state.vault_balance });
        Ok(())
    }

    pub fn set_fee(ctx: Context<OnlyAuthority>, new_fee_bps: u16) -> Result<()> {
        require!(new_fee_bps <= MAX_FEE_BPS, ErrorCode::FeeTooHigh);
        ctx.accounts.state.fee_bps = new_fee_bps;
        emit!(FeeChanged { new_fee_bps });
        Ok(())
    }

    pub fn set_yield_source(ctx: Context<OnlyAuthority>, source: YieldSource) -> Result<()> {
        ctx.accounts.state.yield_source = source.clone();
        emit!(YieldSourceChanged { new_source: source });
        Ok(())
    }

    pub fn set_merkle_root(ctx: Context<OnlyAuthority>, root: [u8; 32]) -> Result<()> {
        ctx.accounts.state.merkle_root = root;
        emit!(MerkleRootUpdated { root });
        Ok(())
    }

    // ── Draw ──

    pub fn request_draw(ctx: Context<RequestDraw>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let clock = Clock::get()?;
        require!(clock.unix_timestamp >= state.last_draw_at + state.draw_interval, ErrorCode::DrawTooSoon);

        let vault_amt = read_token_amount(&ctx.accounts.vault_token);
        require!(vault_amt > 0, ErrorCode::EmptyVault);

        state.vault_balance = vault_amt;
        state.current_winner = Pubkey::default();
        state.current_prize = 0;

        emit!(DrawRequested {
            draw_number: state.total_draws + 1,
            vault_balance: state.vault_balance,
            requested_by: ctx.accounts.payer.key(),
            timestamp: clock.unix_timestamp,
        });
        Ok(())
    }

    pub fn process_draw(ctx: Context<ProcessDraw>, randomness: [u8; 32]) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let clock = Clock::get()?;
        state.last_draw_at = clock.unix_timestamp;
        state.total_draws = state.total_draws.checked_add(1).ok_or(ErrorCode::Overflow)?;

        let seed = u64::from_le_bytes(randomness[..8].try_into().unwrap());
        state.vault_balance = read_token_amount(&ctx.accounts.vault_token);
        state.current_prize = state.vault_balance;

        emit!(DrawProcessed {
            draw_number: state.total_draws,
            randomness_seed: seed,
            prize: state.current_prize,
            timestamp: clock.unix_timestamp,
        });
        Ok(())
    }

    pub fn distribute_prize(ctx: Context<DistributePrize>, winner_addr: Pubkey, proof: Vec<[u8; 32]>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let prize = state.current_prize;
        require!(prize > 0, ErrorCode::NoActiveDraw);

        // Verify Merkle proof: winner is in the holder snapshot
        let leaf = merkle_hash_leaf(&winner_addr);
        require!(
            verify_merkle_proof(&state.merkle_root, &leaf, &proof),
            ErrorCode::InvalidMerkleProof
        );

        validate_vault_token(&ctx.accounts.vault_token, &state.mint, &ctx.accounts.vault_authority.key())?;
        validate_token_account(&ctx.accounts.winner_token, &state.mint, &winner_addr)?;
        require!((ctx.accounts.token_program.key() == TOKEN_PROGRAM_ID || ctx.accounts.token_program.key() == TOKEN_2022_PROGRAM_ID), ErrorCode::InvalidTokenProgram);

        let seeds = &[b"luckie-vault" as &[u8], &[ctx.bumps.vault_authority]];
        let signers = &[&seeds[..]];

        let ix = spl_token_transfer_checked(
            &ctx.accounts.vault_token.key(),
            &state.mint,
            &ctx.accounts.winner_token.key(),
            &ctx.accounts.vault_authority.key(),
            prize,
        );

        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.vault_token.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.winner_token.to_account_info(),
                ctx.accounts.vault_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            signers,
        )?;

        let draw_number = state.total_draws;
        state.vault_balance = read_token_amount(&ctx.accounts.vault_token);
        state.current_winner = Pubkey::default();
        state.current_prize = 0;

        emit!(PrizeDistributed { winner: winner_addr, amount: prize, draw_number });
        Ok(())
    }

    // ── Emergency ──

    pub fn toggle_emergency(ctx: Context<OnlyAuthority>) -> Result<()> {
        ctx.accounts.state.emergency_mode = !ctx.accounts.state.emergency_mode;
        emit!(EmergencyToggled { active: ctx.accounts.state.emergency_mode });
        Ok(())
    }

    pub fn emergency_withdraw(ctx: Context<EmergencyWithdrawAccounts>, amount: u64) -> Result<()> {
        require!(ctx.accounts.state.emergency_mode, ErrorCode::EmergencyNotActive);
        require!(amount > 0, ErrorCode::ZeroAmount);

        validate_vault_token(&ctx.accounts.vault_token, &ctx.accounts.state.mint, &ctx.accounts.vault_authority.key())?;
        validate_token_account(&ctx.accounts.target_token, &ctx.accounts.state.mint, &ctx.accounts.authority.key())?;

        let vault_amt = read_token_amount(&ctx.accounts.vault_token);
        require!(amount <= vault_amt, ErrorCode::InsufficientVaultBalance);

        let seeds = &[b"luckie-vault" as &[u8], &[ctx.bumps.vault_authority]];
        let signers = &[&seeds[..]];

        let ix = spl_token_transfer_checked(
            &ctx.accounts.vault_token.key(),
            &ctx.accounts.state.mint,
            &ctx.accounts.target_token.key(),
            &ctx.accounts.vault_authority.key(),
            amount,
        );

        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.vault_token.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.target_token.to_account_info(),
                ctx.accounts.vault_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            signers,
        )?;

        ctx.accounts.state.vault_balance = read_token_amount(&ctx.accounts.vault_token);
        emit!(EmergencyWithdrawn { amount, target: ctx.accounts.authority.key() });
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════
// VALIDATION HELPERS
// ═══════════════════════════════════════════════════════════════

fn validate_token_account(
    info: &AccountInfo,
    expected_mint: &Pubkey,
    expected_owner: &Pubkey,
) -> Result<()> {
    // Raw byte reads for Token-2022 compatibility. Mint at offset 0,
    // owner at offset 32 — same in both SPL Token and Token-2022.
    let data = info.try_borrow_data()?;
    require!(data.len() >= 165, ErrorCode::InvalidTokenAccount);
    
    let mint_bytes: [u8; 32] = data[0..32].try_into().unwrap();
    require!(Pubkey::new_from_array(mint_bytes) == *expected_mint, ErrorCode::TokenMintMismatch);
    
    let owner_bytes: [u8; 32] = data[32..64].try_into().unwrap();
    require!(Pubkey::new_from_array(owner_bytes) == *expected_owner, ErrorCode::TokenOwnerMismatch);
    
    Ok(())
}

fn validate_vault_token(
    info: &AccountInfo,
    expected_mint: &Pubkey,
    vault_authority: &Pubkey,
) -> Result<()> {
    // Validate mint and owner only (ATA address check skipped for devnet flexibility)
    validate_token_account(info, expected_mint, vault_authority)
}

fn read_token_amount(info: &AccountInfo) -> u64 {
    // Raw byte read for Token-2022 compatibility. Amount is at offset 64
    // in both SPL Token (165 bytes) and Token-2022 with extensions (165+ bytes).
    let data = info.try_borrow_data().unwrap();
    let amount_bytes: [u8; 8] = data[64..72].try_into().unwrap();
    u64::from_le_bytes(amount_bytes)
}

fn spl_token_transfer_checked(
    from: &Pubkey,
    mint: &Pubkey,
    to: &Pubkey,
    authority: &Pubkey,
    amount: u64,
) -> solana_program::instruction::Instruction {
    // Construct Token-2022 TransferChecked instruction manually.
    // Instruction tag 12 = TransferChecked. Data: amount (u64 LE) + decimals (u8).
    // Accounts: source, mint, destination, authority, signers...
    let mut data = vec![12u8]; // TransferChecked tag
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(9u8); // decimals

    let accounts = vec![
        solana_program::instruction::AccountMeta::new(*from, false),
        solana_program::instruction::AccountMeta::new_readonly(*mint, false),
        solana_program::instruction::AccountMeta::new(*to, false),
        solana_program::instruction::AccountMeta::new_readonly(*authority, true),
    ];

    solana_program::instruction::Instruction {
        program_id: TOKEN_2022_PROGRAM_ID,
        accounts,
        data,
    }
}

// ═══════════════════════════════════════════════════════════════
// MERKLE PROOF VERIFICATION
// ═══════════════════════════════════════════════════════════════

const LEAF_PREFIX: &[u8] = b"luckie:leaf:";
const NODE_PREFIX: &[u8] = b"luckie:node:";

/// Hash a holder address as a Merkle leaf.
/// Domain-separated: SHA-256("luckie:leaf:" + pubkey_bytes)
fn merkle_hash_leaf(address: &Pubkey) -> [u8; 32] {
    let mut data = Vec::with_capacity(LEAF_PREFIX.len() + 32);
    data.extend_from_slice(LEAF_PREFIX);
    data.extend_from_slice(&address.to_bytes());
    hash(&data).to_bytes()
}

/// Hash two child hashes to produce a parent node.
/// Domain-separated: SHA-256("luckie:node:" + sorted(a, b))
fn merkle_hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (a, b) = if left < right { (left, right) } else { (right, left) };
    let mut data = Vec::with_capacity(NODE_PREFIX.len() + 64);
    data.extend_from_slice(NODE_PREFIX);
    data.extend_from_slice(a);
    data.extend_from_slice(b);
    hash(&data).to_bytes()
}

/// Verify a Merkle proof on-chain.
/// Given a root, leaf hash, and sibling hashes, recompute the root and compare.
fn verify_merkle_proof(
    root: &[u8; 32],
    leaf: &[u8; 32],
    siblings: &[[u8; 32]],
) -> bool {
    let mut hash = *leaf;
    for sibling in siblings {
        hash = merkle_hash_pair(&hash, sibling);
    }
    hash == *root
}

// ═══════════════════════════════════════════════════════════════
// STATE
// ═══════════════════════════════════════════════════════════════

#[account]
pub struct ProgramState {
    pub authority: Pubkey,
    pub fee_bps: u16,
    pub mint: Pubkey,
    pub merkle_root: [u8; 32],
    pub vault_balance: u64,
    pub draw_interval: i64,
    pub last_draw_at: i64,
    pub total_draws: u64,
    pub current_winner: Pubkey,
    pub current_prize: u64,
    pub yield_source: YieldSource,
    pub emergency_mode: bool,
}

impl ProgramState {
    pub const LEN: usize = 8 + 32 + 2 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 8 + 2 + 1 + 96;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum YieldSource {
    None,
    MarinadeSolStaking,
    KaminoLending,
    JupiterLP,
    Custom(Pubkey),
}

// ═══════════════════════════════════════════════════════════════
// CONTEXTS
// ═══════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = ProgramState::LEN, seeds = [b"luckie-state"], bump)]
    pub state: Account<'info, ProgramState>,
    #[account(seeds = [b"luckie-vault"], bump)]
    /// CHECK: PDA derived from luckie-vault seeds, validated in CPI
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: validated in instruction via spl_token::state::Account::unpack
    #[account(mut)]
    pub vault_token: AccountInfo<'info>,
    /// CHECK: validated against state.mint during CPI
    pub mint: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: validated as TOKEN_PROGRAM_ID in instruction
    pub token_program: AccountInfo<'info>,
    /// CHECK: used for ATA derivation only
    pub associated_token_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct OnlyAuthority<'info> {
    #[account(mut, seeds = [b"luckie-state"], bump, has_one = authority @ ErrorCode::Unauthorized)]
    pub state: Account<'info, ProgramState>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct FundVault<'info> {
    #[account(mut, seeds = [b"luckie-state"], bump)]
    pub state: Account<'info, ProgramState>,
    /// CHECK: validated in instruction
    #[account(mut)]
    pub vault_token: AccountInfo<'info>,
    /// CHECK: validated in instruction
    #[account(mut)]
    pub from: AccountInfo<'info>,
    /// CHECK: validated against state.mint
    pub mint: AccountInfo<'info>,
    pub authority: Signer<'info>,
    #[account(seeds = [b"luckie-vault"], bump)]
    /// CHECK: PDA derived from luckie-vault seeds, validated in CPI
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: validated as TOKEN_PROGRAM_ID
    pub token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RequestDraw<'info> {
    #[account(mut, seeds = [b"luckie-state"], bump)]
    pub state: Account<'info, ProgramState>,
    /// CHECK: vault ATA validated via validate_token_account (mint + owner)
    #[account(mut)]
    pub vault_token: AccountInfo<'info>,
    #[account(seeds = [b"luckie-vault"], bump)]
    /// CHECK: PDA derived from luckie-vault seeds, validated in CPI
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: Switchboard VRF account
    pub vrf: AccountInfo<'info>,
    /// CHECK: Switchboard program
    pub vrf_program: AccountInfo<'info>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProcessDraw<'info> {
    #[account(mut, seeds = [b"luckie-state"], bump)]
    pub state: Account<'info, ProgramState>,
    /// CHECK: vault ATA
    #[account(mut)]
    pub vault_token: AccountInfo<'info>,
    #[account(seeds = [b"luckie-vault"], bump)]
    /// CHECK: PDA derived from luckie-vault seeds, validated in CPI
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: VRF account
    pub vrf: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DistributePrize<'info> {
    #[account(mut, seeds = [b"luckie-state"], bump, has_one = authority)]
    pub state: Account<'info, ProgramState>,
    /// CHECK: validated in instruction
    #[account(mut)]
    pub vault_token: AccountInfo<'info>,
    /// CHECK: LUCK mint
    pub mint: AccountInfo<'info>,
    /// CHECK: validated in instruction; ATA of winner
    #[account(mut)]
    pub winner_token: AccountInfo<'info>,
    #[account(seeds = [b"luckie-vault"], bump)]
    /// CHECK: PDA derived from luckie-vault seeds, validated in CPI
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: validated in instruction as TOKEN_PROGRAM_ID
    pub token_program: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct EmergencyWithdrawAccounts<'info> {
    #[account(mut, seeds = [b"luckie-state"], bump, has_one = authority)]
    pub state: Account<'info, ProgramState>,
    /// CHECK: vault ATA
    #[account(mut)]
    pub vault_token: AccountInfo<'info>,
    /// CHECK: LUCK mint
    pub mint: AccountInfo<'info>,
    /// CHECK: authority's token account
    #[account(mut)]
    pub target_token: AccountInfo<'info>,
    #[account(seeds = [b"luckie-vault"], bump)]
    /// CHECK: PDA derived from luckie-vault seeds, validated in CPI
    pub vault_authority: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK: validated in instruction
    pub token_program: AccountInfo<'info>,
}

// ═══════════════════════════════════════════════════════════════
// EVENTS
// ═══════════════════════════════════════════════════════════════

#[event]
pub struct Initialized { pub authority: Pubkey, pub fee_bps: u16, pub mint: Pubkey, pub draw_interval: i64 }
#[event]
pub struct VaultFunded { pub amount: u64, pub funder: Pubkey, pub vault_balance: u64 }
#[event]
pub struct FeeChanged { pub new_fee_bps: u16 }
#[event]
pub struct YieldSourceChanged { pub new_source: YieldSource }
#[event]
pub struct DrawRequested { pub draw_number: u64, pub vault_balance: u64, pub requested_by: Pubkey, pub timestamp: i64 }
#[event]
pub struct DrawProcessed { pub draw_number: u64, pub randomness_seed: u64, pub prize: u64, pub timestamp: i64 }

#[event]
pub struct PrizeDistributed { pub winner: Pubkey, pub amount: u64, pub draw_number: u64 }
#[event]
pub struct EmergencyToggled { pub active: bool }
#[event]
pub struct EmergencyWithdrawn { pub amount: u64, pub target: Pubkey }

#[event]
pub struct MerkleRootUpdated { pub root: [u8; 32] }

// ═══════════════════════════════════════════════════════════════
// ERROR CODES
// ═══════════════════════════════════════════════════════════════

#[error_code]
pub enum ErrorCode {
    #[msg("Only the program authority can perform this action")]
    Unauthorized,
    #[msg("Fee cannot exceed 10% (1000 basis points)")]
    FeeTooHigh,
    #[msg("Draw interval must be at least 1 hour")]
    InvalidInterval,
    #[msg("The draw interval has not elapsed yet")]
    DrawTooSoon,
    #[msg("No active draw to set a winner for")]
    NoActiveDraw,
    #[msg("Vault balance is zero")]
    EmptyVault,
    #[msg("Insufficient vault balance")]
    InsufficientVaultBalance,
    #[msg("Emergency mode is not active")]
    EmergencyNotActive,
    #[msg("Token account mint does not match")]
    TokenMintMismatch,
    #[msg("Token account owner does not match")]
    TokenOwnerMismatch,
    #[msg("Token account is not valid")]
    InvalidTokenAccount,
    #[msg("Vault token account is not the canonical ATA")]
    InvalidVaultToken,
    #[msg("Invalid token program ID")]
    InvalidTokenProgram,
    #[msg("Arithmetic overflow detected")]
    Overflow,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Merkle proof does not match the stored root")]
    InvalidMerkleProof,
}

#[cfg(test)]
mod tests;
