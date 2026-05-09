# LuckieToken — Development Roadmap

## Architecture

LuckieToken is a Token-2022 mint with a 3% transfer fee that feeds a program-controlled vault. A Merkle-tree snapshot of holders enables verifiable random draws where the prize is pushed directly to the winner.

## Current Status

- Program deployed on Solana devnet
- Token-2022 mint with 3% transfer fee
- PDA-controlled vault with manual funding
- Merkle proof verification on-chain
- Push-based prize distribution
- 19 unit tests passing
- End-to-end simulation validated

## Phases

### Phase 1 — Contract Foundation ✅

- Anchor program with vault, draw, and prize distribution
- Merkle tree snapshot for holder verification
- Emergency circuit breaker
- Token-2022 integration with automatic transfer fees
- 19 tests passing

### Phase 2 — Yield Integration

- Marinade SOL staking
- Kamino lending
- Jupiter LP
- Automated yield harvesting

### Phase 3 — VRF & Draw Scheduling

- Switchboard VRF integration for verifiable randomness
- Automated draw scheduling
- On-chain holder snapshot automation

### Phase 4 — Frontend

- Web dashboard (vault stats, draw history, winner list)
- Admin panel for authority operations
- Integration tests (devnet to mainnet)
- Security audit

## Devnet Addresses

| Resource | Address |
|----------|---------|
| Program | `23u6C2yNfpoxu66bAPgoiEsCSFHrMRdGQYCTYg5zsoGg` |
| Token Mint | `79ELFdDfkYB6gALuPZxsmaPrU7i5P6eKW54ZoZS1yZhB` |
| Authority | `MxZC6m8X85qWgXFhSHc5xEDyMMgprKcBiSvb8Zt9Jbo` |
