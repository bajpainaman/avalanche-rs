# avalanche-rs Agent Guide

Guide for AI agents operating on this codebase on Naman's machine.

---

## Environment

| Tool | Version | Path |
|------|---------|------|
| Rust | 1.88.0 | Homebrew |
| Cargo | 1.88.0 | Homebrew |
| Go | 1.25.6 | darwin/arm64 |
| protoc | 33.4 | Required for proto builds |

**Working directory**: `/Users/namanbajpai/avalanche-rs`

**Git remotes**:
- `origin` = `ava-labs/avalanche-rs` (upstream, read-only)
- `fork` = `bajpainaman/avalanche-rs` (push here)

**Current branch**: `feat/granite-upgrade-conformance` (Granite/ACP-77 L1 validator support)

---

## Workspace Layout

```
avalanche-rs/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── avalanche-types/          # Core SDK crate (most work happens here)
│   │   ├── src/
│   │   │   ├── ids/              # ID types (32-byte, 20-byte node, short)
│   │   │   ├── key/              # Cryptography (secp256k1, secp256r1, BLS)
│   │   │   ├── platformvm/       # P-Chain types
│   │   │   │   ├── txs/          # Transaction types (including L1 txs)
│   │   │   │   ├── l1/           # L1 validator types (Granite)
│   │   │   │   └── fees/         # ACP-103 fee complexity
│   │   │   ├── warp/             # Warp/ICM messages (Granite)
│   │   │   ├── avm/              # X-Chain types
│   │   │   ├── evm/              # C-Chain / EVM types
│   │   │   ├── jsonrpc/          # JSON-RPC client for all chains
│   │   │   ├── message/          # P2P message serialization
│   │   │   ├── packer/           # Binary serialization (Go-compatible)
│   │   │   └── subnet/           # VM SDK (gRPC server/client)
│   │   ├── examples/             # 33 runnable examples
│   │   └── Cargo.toml            # Feature flags defined here
│   └── avalanche-consensus/      # Snowman/Snowball consensus
├── core/
│   ├── cert-manager/             # TLS certificate management
│   ├── network/                  # P2P networking layer
│   └── server/                   # gRPC server utilities
├── avalanchego-conformance/      # Go reference server (avalanchego v1.14.0)
├── avalanchego-conformance-sdk/  # Rust gRPC client for conformance tests
├── tests/
│   ├── avalanche-e2e/            # E2E tests (needs running node)
│   ├── avalanchego-byzantine/    # Byzantine fault tolerance tests
│   └── avalanchego-conformance/  # Byte-level conformance tests
├── scripts/                      # Test/build scripts
└── docs/
    └── GRANITE_UPGRADE.md        # Granite upgrade technical docs
```

---

## Quick Commands

### Build

```bash
# Build default members (avalanche-types + avalanche-consensus)
cargo build

# Build with all features
cargo build --all-features

# Build entire workspace
cargo build --workspace
```

### Test

```bash
# Unit tests (fast, no external dependencies)
cargo test --workspace

# Unit tests with all features enabled
RUST_LOG=debug cargo test --all-features -p avalanche-types -p avalanche-consensus

# Or use the script
./scripts/tests.unit.sh
```

### Conformance Tests (requires Go server)

```bash
# Step 1: Build and start the Go conformance server
cd avalanchego-conformance && go install -v ./cmd/avalanchego-conformance && cd ..
avalanchego-conformance server --log-level debug --port=22342 --grpc-gateway-port=22343 &

# Step 2: Run Rust conformance tests
AVALANCHEGO_CONFORMANCE_SERVER_RPC_ENDPOINT=http://127.0.0.1:22342 \
  cargo test --all-features --package avalanchego-conformance -- --show-output

# Or use the script (does both steps)
./scripts/tests.avalanchego-conformance.sh
```

### Lint

```bash
./scripts/tests.lint.sh
```

---

## Feature Flags

All features in `avalanche-types` are **opt-in** (default = none). Enable what you need:

| Feature | What It Enables | Key Dependencies |
|---------|----------------|------------------|
| `jsonrpc_client` | JSON-RPC clients for C/X/P chains | reqwest, tokio |
| `evm` | EVM transaction building & signing | ethers, rlp |
| `wallet_evm` | High-level EVM wallet abstraction | ethers full |
| `kms_aws` | AWS KMS key management | aws-sdk-kms |
| `mnemonic` | BIP-32 mnemonic key derivation | bip32 |
| `message` | P2P message serialization | flate2, proto |
| `proto` | Protocol buffer types | prost, tonic |
| `subnet` | VM SDK with gRPC server/client | tonic, tokio-stream |
| `secp256r1` | P-256 curve (Granite/ACP-204) | p256 |
| `libsecp256k1` | Alternative secp256k1 impl | secp256k1 |

To build/test with specific features:
```bash
cargo test -p avalanche-types --features "jsonrpc_client,evm,wallet_evm"
```

---

## Key Modules Reference

### IDs (`src/ids/`)
- `ids::Id` - 32-byte identifier (tx IDs, block IDs, chain IDs)
- `ids::node::Id` - 20-byte node identifier
- `ids::short::Id` - 20-byte short identifier (addresses)

### Packer (`src/packer/mod.rs`)
Binary serialization matching Go's avalanchego packer. Two header modes:
- `pack_bytes_with_header()` - u32 length prefix (standard)
- `pack_bytes_with_header_u64()` - u64 length prefix (Warp messages)
- `pack_bytes()` - no length prefix (fixed-size fields like BLS signatures)

### Warp Messages (`src/warp/`)
- `message::UnsignedMessage` - network_id + source_chain_id + payload
- `message::Message` - unsigned + bitset + BLS signature (96 bytes, no header)
- `payload::*` - L1 validator management payloads (4 types)

### L1 Transactions (`src/platformvm/txs/`)
Five Granite transaction types, all implement `to_bytes()` and `sign()`:
- `convert_subnet_to_l1::Tx`
- `register_l1_validator::Tx`
- `set_l1_validator_weight::Tx`
- `increase_l1_validator_balance::Tx`
- `disable_l1_validator::Tx`

### JSON-RPC Clients (`src/jsonrpc/client/`)
- `evm.rs` - C-Chain (chain_id, get_balance, get_nonce, send_raw_tx)
- `p.rs` - P-Chain (validators, subnets, blockchains, height)
- `x.rs` - X-Chain (get_balance, get_utxos, get_asset_description)
- `info.rs` - Node info (network_name, node_id, peers, is_bootstrapped)
- `health.rs` - Node health checks

---

## Serialization Rules (Critical)

When working with binary serialization, these rules MUST be followed to maintain Go compatibility:

1. **Standard fields**: Use `pack_bytes_with_header()` (u32 length prefix)
2. **Warp bitsets**: Use `pack_bytes_with_header_u64()` (u64 length prefix)
3. **BLS signatures**: Use `pack_bytes()` directly (always 96 bytes, NO header)
4. **Codec version**: Always `pack_u16(0)` first when serializing full transactions
5. **Type ID**: `pack_u32(type_id)` after codec version
6. **Field order**: Must match Go struct field order exactly

---

## Running Examples

Examples require specific features. Pattern:

```bash
# Query node info (just needs jsonrpc_client)
cargo run -p avalanche-types \
  --features jsonrpc_client \
  --example jsonrpc_client_info -- \
  --http-url http://localhost:9650

# Send EVM transaction (needs jsonrpc_client + evm)
cargo run -p avalanche-types \
  --features "jsonrpc_client,evm" \
  --example evm_send_raw_transaction_eip1559_hot_key

# Generate a key
cargo run -p avalanche-types \
  --example key_secp256k1_info_gen
```

---

## Git Workflow

```bash
# Always push to fork, not origin
git push fork <branch-name>

# PRs go from fork -> origin/main
gh pr create --repo ava-labs/avalanche-rs \
  --head bajpainaman:<branch> \
  --base main
```

**Branch naming**: `feat/<description>` or `fix/<description>`
**Commit style**: conventional commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`)

---

## Common Pitfalls

1. **Feature flags off by default** - If you get "unresolved import" errors, you probably need to enable a feature flag. Check the feature table above.

2. **Conformance server not running** - Integration tests that need the Go conformance server will skip gracefully with a log message. They won't fail.

3. **Go binary location** - After `go install`, the binary is at `$GOPATH/bin/avalanchego-conformance`. Make sure `$GOPATH/bin` is in `$PATH`.

4. **Warp serialization** - The BLS signature in Warp messages has NO length header. This was a bug that was fixed. Don't add one.

5. **proto regeneration** - If you modify `.proto` files in `avalanchego-conformance/rpcpb/`, you need to rebuild the Go server AND the Rust bindings (handled by `build.rs` in `avalanchego-conformance-sdk`).

---

## Test Counts (as of current branch)

| Crate | Tests | Status |
|-------|-------|--------|
| avalanche-types | 213 | All passing |
| avalanche-consensus | 133 | All passing |
| avalanchego-conformance | 59 | All passing |
| avalanchego-conformance-sdk | 7 | All passing |
| Other crates | 46 | All passing (2 ignored) |
| **Total** | **458** | **0 failures** |

---

## Documentation

- **Granite upgrade details**: `docs/GRANITE_UPGRADE.md`
- **This guide**: `docs/AGENT_GUIDE.md`
- **Crate docs**: `cargo doc --all-features --open`
