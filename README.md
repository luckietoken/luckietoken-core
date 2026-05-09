# LuckieToken Core

A decentralized **Hold-to-Win** token on Solana. Every transaction feeds a prize vault, and a randomly selected holder wins the accumulated rewards — no claim required.

## How It Works

```
1. Transfer Tax (3%)  -->  2. Vault Accumulates  -->  3. Merkle Snapshot
                                  |
                          4. Random Draw
                                  |
                          5. Prize pushed to winner
```

- **Step 1:** Every LUCK transfer incurs a 3% tax via Token-2022
- **Step 2:** Taxes accumulate in a program-owned Vault (PDA)
- **Step 3:** A holder snapshot is taken and committed on-chain as a Merkle root
- **Step 4:** A random draw selects a winner from the holder set
- **Step 5:** The prize is pushed directly to the winner — no claim required

## Key Features

- **Secure by design:** Raw byte validation for Token-2022 compatibility
- **PDA-controlled vault:** Only the program can authorize vault transfers
- **Emergency circuit breaker:** Authority-gated emergency withdrawal
- **Merkle proof verification:** On-chain proof that the winner is a verified holder
- **Domain-separated hashing:** `luckie:leaf:` and `luckie:node:` prefixes prevent collision attacks
- **Open source:** MIT license, community-auditable

## Project Structure

```
luckietoken-core/
├── programs/luckietoken/        # Anchor program (Rust)
│   └── src/
│       ├── lib.rs               # Core contract logic
│       └── tests.rs             # 19 unit tests
├── src/utils/                   # TypeScript utilities
│   ├── hash.ts                  # SHA-256 with domain separation
│   └── merkle.ts                # Merkle tree builder + proof generator
├── scripts/
│   └── simulate.js              # Full simulation (holders -> Merkle -> draw -> prize)
├── tests/
│   └── test_integrity.ts        # Cross-language hash verification
├── docs/
│   └── ROADMAP.md               # Development roadmap
├── Anchor.toml                  # Anchor configuration
├── Cargo.toml                   # Rust workspace
└── package.json                 # Node.js dependencies
```

## Devnet Addresses

| Resource | Address |
|----------|---------|
| Program | `23u6C2yNfpoxu66bAPgoiEsCSFHrMRdGQYCTYg5zsoGg` |
| Token Mint | `79ELFdDfkYB6gALuPZxsmaPrU7i5P6eKW54ZoZS1yZhB` (Token-2022) |
| Transfer Fee | 3% (300 bps) |

## Quick Start

```bash
# Prerequisites
solana --version        # >= 2.1.0
rustc --version         # >= 1.85
node --version          # >= 18

# Install dependencies
npm install

# Build the program
npm run build

# Deploy to devnet
npm run deploy

# Run full simulation
npm run simulate
```

## License

MIT — see [LICENSE](./LICENSE)
