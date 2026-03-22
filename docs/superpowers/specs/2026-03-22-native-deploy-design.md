# Native Deploy with Priority Fees

## What We're Building

Replace all four `solana` CLI shell-outs in the deploy pipeline with native Rust RPC calls. Add automatic priority fee calculation with manual override. Add pre-deploy validation that catches authority mismatches before wasting time and SOL on buffer uploads.

After this work, `quasar deploy` no longer requires the Solana CLI to be installed. The only external dependency is a Solana RPC endpoint.

## Why This Approach

The current implementation shells out to `solana program deploy`, `write-buffer`, `set-buffer-authority`, and `set-upgrade-authority`. This works but has problems:

- Requires the Solana CLI as a runtime dependency
- No priority fee support on the shell-out paths (only on the native Squads proposal path)
- Errors surface as parsed stdout/stderr strings rather than structured errors
- No pre-validation — a mismatched upgrade authority wastes the entire buffer upload before failing
- No progress visibility during buffer upload

Anchor solved this by going fully native. We already have the infrastructure (ureq, solana-transaction, solana-instruction, ed25519-dalek, bincode) from the Squads integration. Extending it to cover BPF Loader Upgradeable instructions is straightforward.

## Key Decisions

- **Sequential buffer chunk uploads** for v1 (not parallel). Simpler, reliable, can add parallelism later.
- **Priority fees: auto-calculated with manual override.** `getRecentPrioritizationFees` → median, with `--priority-fee <micro_lamports>` flag to override.
- **Strict `--upgrade` validation both directions.** `--upgrade` on a non-existent program errors. No `--upgrade` on an existing program errors. No silent auto-detection.
- **Split multisig.rs into three modules.** `rpc.rs` (RPC helpers + priority fees + Keypair), `bpf_loader.rs` (BPF loader instructions + buffer upload + deploy/upgrade), `multisig.rs` (Squads-only logic).

## Module Structure

### `rpc.rs` — RPC Client & Signing Helpers

Extracted from current `multisig.rs`. Responsibilities: talk to a Solana RPC node, manage keypairs and config.

**Moved from multisig.rs:**
- `Keypair` struct (ed25519-dalek wrapper implementing `solana_signer::Signer`) — used by both `bpf_loader.rs` and `multisig.rs`
- `read_program_id_from_keypair(path) -> Address` — reads public key from keypair file
- `get_latest_blockhash(rpc_url) -> Hash`
- `send_transaction(rpc_url, tx_bytes) -> String` (signature)
- `get_account_data(rpc_url, address) -> Option<Vec<u8>>`
- `program_exists_on_chain(rpc_url, program_id) -> bool`
- `solana_rpc_url(url_override) -> String` (cluster resolution)
- `solana_keypair_path(keypair_override) -> PathBuf`
- `read_config_field(field) -> Option<String>`
- `expand_tilde(path) -> String`
- `resolve_cluster(input) -> String`

**New:**
- `get_recent_prioritization_fees(rpc_url) -> u64` — calls `getRecentPrioritizationFees`, returns median fee in micro-lamports. Returns 0 if no recent fees.
- `confirm_transaction(rpc_url, signature, timeout_secs) -> bool` — polls `getSignatureStatuses` every 500ms until `confirmed` commitment or timeout
- `get_minimum_balance_for_rent_exemption(rpc_url, data_len) -> u64` — calls `getMinimumBalanceForRentExemption`

### `bpf_loader.rs` — BPF Loader Upgradeable

All interactions with the BPF Loader Upgradeable program. No Squads knowledge.

**Constants (pub, moved from multisig.rs):**
- `BPF_LOADER_UPGRADEABLE_ID`
- `SYSTEM_PROGRAM_ID`
- `SYSVAR_RENT_ID`
- `SYSVAR_CLOCK_ID`
- `COMPUTE_BUDGET_PROGRAM_ID` (new: `ComputeBudget111111111111111111111111111111`)
- `CHUNK_SIZE: usize = 950` — bytes per Write transaction. Accounts for transaction overhead (~212 bytes) plus ComputeBudget::SetComputeUnitPrice instruction (~45 bytes) within the 1232-byte transaction limit.
- `BUFFER_HEADER_SIZE: usize = 37` — 4 bytes (UpgradeableLoaderState::Buffer discriminant, u32 LE = 1) + 1 byte (Option tag) + 32 bytes (authority pubkey)

**BPF Loader instructions (5 variants, u32 LE discriminant):**
- `initialize_buffer_ix(buffer, authority) -> Instruction`
  - Discriminant: `0u32` LE. Accounts: [0] buffer (writable), [1] authority (readonly)
- `write_ix(buffer, authority, offset, data) -> Instruction`
  - Discriminant: `1u32` LE. Data: 4 bytes disc + 4 bytes offset (u32 LE) + 4 bytes len (u32 LE) + bytes. Accounts: [0] buffer (writable), [1] authority (signer)
- `deploy_with_max_data_len_ix(payer, programdata, program, buffer, authority, data_len) -> Instruction`
  - Discriminant: `2u32` LE. Data: 4 bytes disc + 8 bytes max_data_len (usize/u64 LE). Accounts: [0] payer (writable signer), [1] programdata (writable), [2] program (writable), [3] buffer (writable), [4] rent sysvar, [5] clock sysvar, [6] system program, [7] BPF loader (program)
- `upgrade_ix(programdata, program, buffer, spill, authority) -> Instruction`
  - Discriminant: `3u32` LE. Accounts: [0] programdata (writable), [1] program (writable), [2] buffer (writable), [3] spill (writable), [4] rent sysvar, [5] clock sysvar, [6] authority (signer)
- `set_authority_ix(account, current_authority, new_authority: Option<Address>) -> Instruction`
  - Discriminant: `4u32` LE. Accounts: [0] buffer or programdata (writable), [1] current authority (signer), [2] new authority (readonly, optional — present = transfer, absent = revoke/make immutable). Data: just the 4-byte discriminant.

**Compute budget instruction:**
- `set_compute_unit_price_ix(micro_lamports: u64) -> Instruction`
  - Program: `COMPUTE_BUDGET_PROGRAM_ID`. Instruction discriminant: `3u8`. Data: 1 byte disc + 8 bytes micro_lamports (u64 LE). No accounts.

**Programdata account layout** (for authority verification):
```
[0..4]   u32 LE = 3 (UpgradeableLoaderState::ProgramData discriminant)
[4..12]  u64 LE slot (deployment slot)
[12]     u8 Option tag (0 = None/immutable, 1 = Some)
[13..45] [u8; 32] authority pubkey (only valid if Option tag = 1)
```

**Buffer upload:**
- `write_buffer(so_path, payer: &Keypair, rpc_url, priority_fee) -> Address`
  1. Reads .so file into memory
  2. Generates a random `Keypair` for the buffer account (using `ed25519-dalek` + `rand`)
  3. Queries `get_minimum_balance_for_rent_exemption(so_file.len() + BUFFER_HEADER_SIZE)`
  4. Sends a transaction with: [SetComputeUnitPrice, SystemProgram::CreateAccount, InitializeBuffer]. Signed by **both** payer and buffer keypair (buffer must sign its own CreateAccount).
  5. Calls `confirm_transaction` to wait for confirmation
  6. For each 950-byte chunk: sends [SetComputeUnitPrice, Write(buffer, payer, offset, chunk)]. Signed by payer only. Waits for confirmation before sending next chunk.
  7. Shows `indicatif` progress bar (bytes written / total)
  8. Returns buffer address

**Deploy orchestrator:**
- `deploy_program(so_path, program_keypair: &Keypair, payer: &Keypair, rpc_url, priority_fee) -> Address`
  1. Calls `write_buffer` to upload the .so
  2. Derives programdata PDA from program address
  3. Sends a transaction with: [SetComputeUnitPrice, SystemProgram::CreateAccount(program, 36 bytes, BPF loader owner), DeployWithMaxDataLen]. The program account is 36 bytes: 4-byte discriminant + 32-byte programdata address. Signed by **both** payer and program keypair (program must sign its own CreateAccount).
  4. Confirms transaction
  5. Returns program address

  Note: `program_keypair` is a `Keypair` (signer), not an `Address`, because `DeployWithMaxDataLen` requires the program account to sign the `CreateAccount` that allocates it.

**Upgrade orchestrator:**
- `upgrade_program(so_path, program_id: &Address, authority: &Keypair, rpc_url, priority_fee)`
  1. Calls `write_buffer` to upload the .so
  2. Derives programdata PDA from program address
  3. Sends a transaction with: [SetComputeUnitPrice, Upgrade(programdata, program, buffer, authority/spill, authority)]. Signed by authority.
  4. Confirms transaction

**Validation:**
- `verify_upgrade_authority(rpc_url, program_id, expected_authority: &Address) -> Result<()>`
  1. Derives programdata PDA via `programdata_pda(program_id)`
  2. Fetches programdata account via `get_account_data`
  3. Reads Option tag at byte 12: if 0, error "program is immutable"
  4. Reads authority pubkey at bytes 13..45
  5. Compares to `expected_authority`. If mismatch, error: "upgrade authority mismatch: on-chain is X, your keypair is Y"

**PDA derivation (moved from multisig.rs):**
- `programdata_pda(program_id) -> (Address, u8)` — `Address::find_program_address(&[program_id], &BPF_LOADER_UPGRADEABLE_ID)`

### `multisig.rs` — Squads Integration (Reduced)

Keeps only Squads-specific logic. Imports from `crate::rpc` and `crate::bpf_loader`.

**Stays:**
- Squads constants (`SQUADS_PROGRAM_ID`)
- PDA derivation (`vault_pda`, `transaction_pda`, `proposal_pda`)
- Squads instruction builders (`vault_transaction_create_ix`, `proposal_create_ix`, `proposal_approve_ix`, `vault_transaction_execute_ix`)
- `build_upgrade_message` (imports sysvar constants from `bpf_loader`)
- Account parsing (`parse_multisig_account`, `parse_proposal_account`, `read_transaction_index`)
- Data types (`MultisigMember`, `MultisigState`, `ProposalStatus`, `ProposalState`)
- `propose_upgrade` orchestrator — updated signature to accept `priority_fee: Option<u64>`
- `show_proposal_status` / `execute_approved_proposal`
- `short_addr`
- `anchor_discriminator`

**Removed (moved to rpc.rs or bpf_loader.rs):**
- All RPC helpers → `rpc.rs`
- `Keypair` struct → `rpc.rs`
- `read_program_id_from_keypair` → `rpc.rs`
- `BPF_LOADER_UPGRADEABLE_ID`, `SYSTEM_PROGRAM_ID`, sysvar constants → `bpf_loader.rs`
- `programdata_pda` → `bpf_loader.rs`
- `write_buffer` shell-out → replaced by `bpf_loader::write_buffer()`
- `set_buffer_authority` shell-out → replaced by `bpf_loader::set_authority()`
- `set_upgrade_authority` shell-out → replaced by `bpf_loader::set_authority()`

**Updated:**
- `propose_upgrade` gains `priority_fee: Option<u64>` parameter. Calls `bpf_loader::write_buffer()` and `bpf_loader::set_authority()` instead of shell-outs. Passes priority fee through.
- `build_upgrade_message` uses `bpf_loader::SYSVAR_RENT_ID`, `bpf_loader::SYSVAR_CLOCK_ID`, `bpf_loader::BPF_LOADER_UPGRADEABLE_ID`
- `execute_approved_proposal` gains `priority_fee: Option<u64>` parameter, passes through to transaction building

### `deploy.rs` — Command Entry Point (Updated)

Routing logic stays the same (4 code paths). Changes:
- `solana_deploy()` shell-out function **removed entirely**
- Fresh deploy path calls `bpf_loader::deploy_program()`
- Upgrade path calls `bpf_loader::upgrade_program()`
- Authority transfer calls `bpf_loader::set_authority()`
- **New reverse check:** `if upgrade && !program_exists_on_chain(...)` → error "program not found at X, drop --upgrade for a fresh deploy"
- **New authority validation:** before upgrade, calls `bpf_loader::verify_upgrade_authority()` to catch mismatch before buffer upload
- Resolves priority fee (auto or override) and passes through all code paths
- Destructures `priority_fee` from `DeployOpts` and propagates to all orchestrators
- Removes `use std::process::{Command, Stdio}` (no more shell-outs)

### `lib.rs` — CLI Definition

New module declarations:
```rust
pub mod bpf_loader;
pub mod rpc;
```

One new field on `DeployCommand`:
```rust
/// Priority fee in micro-lamports (auto-calculated if omitted)
#[arg(long, value_name = "MICRO_LAMPORTS")]
pub priority_fee: Option<u64>,
```

`DeployOpts` gets a matching `priority_fee: Option<u64>` field. `deploy::run()` destructures it and passes through.

## Deploy Flow (After)

### Fresh deploy: `quasar deploy`

1. Build .so (unless `--skip-build`)
2. Resolve cluster URL
3. Check program doesn't exist on-chain → error if it does
4. Calculate priority fee (auto or override)
5. Create buffer account (random keypair) + write chunks sequentially with progress bar, confirming each
6. Create program account (from program keypair) + DeployWithMaxDataLen in one transaction
7. Print program ID

### Upgrade: `quasar deploy --upgrade`

1. Build .so
2. Resolve cluster URL
3. Check program exists on-chain → error if it doesn't (new)
4. Verify upgrade authority matches keypair → error if mismatch (new, before buffer upload)
5. Calculate priority fee
6. Create buffer + write chunks sequentially with progress bar, confirming each
7. Send Upgrade transaction, confirm
8. Print success

### Multisig fresh deploy: `quasar deploy --multisig <ADDR>`

1. Build .so
2. Deploy via native `bpf_loader::deploy_program()` (was shell-out)
3. Transfer authority to vault via native `bpf_loader::set_authority()` (was shell-out)

### Multisig upgrade: `quasar deploy --upgrade --multisig <ADDR>`

1. Build .so
2. Write buffer via native `bpf_loader::write_buffer()` (was shell-out)
3. Transfer buffer authority to vault via native `bpf_loader::set_authority()` (was shell-out)
4. Create Squads proposal (already native, now with priority fee)

## Priority Fee Flow

1. If `--priority-fee <N>` is set, use N micro-lamports
2. Otherwise, call `getRecentPrioritizationFees` RPC method with no accounts filter
3. Collect the `prioritizationFee` values from the response
4. Take the median (sort, pick middle value). If empty, use 0.
5. Prepend `SetComputeUnitPrice(fee)` as the first instruction in every transaction (buffer writes, deploy, upgrade, authority changes, Squads proposals)

## Buffer Upload Detail

The .so binary is written to a buffer account in chunks:

1. **Generate buffer keypair:** Random `Keypair` via `ed25519-dalek` + `rand` (both already in Cargo.toml)
2. **Create buffer account:** Transaction with [SetComputeUnitPrice, SystemProgram::CreateAccount (rent-exempt for data_len + 37 header bytes, owned by BPF Loader), InitializeBuffer]. Signed by payer + buffer keypair. Wait for `confirmed` via `confirm_transaction`.
3. **Write chunks:** For each 950-byte chunk, send [SetComputeUnitPrice, Write(buffer, payer, offset, chunk)]. Signed by payer only. Call `confirm_transaction` after each to ensure sequential ordering.
4. **Progress bar:** `indicatif` ProgressBar showing bytes written / total bytes

The 37-byte header is the BPF Loader buffer account structure: 4 bytes (UpgradeableLoaderState discriminant, u32 LE = 1 for Buffer) + 1 byte (Option tag) + 32 bytes (authority pubkey).

If a chunk write fails, the error includes the buffer address so the user can close it with `solana program close <BUFFER>` to reclaim rent.

## Error Paths

| Scenario | Behavior |
|---|---|
| `quasar deploy` on existing program | Error: "program already deployed at X, use --upgrade" (existing) |
| `quasar deploy --upgrade` on non-existent program | Error: "program not found at X, drop --upgrade for a fresh deploy" (new) |
| `quasar deploy --upgrade` with wrong authority keypair | Error before buffer upload: "upgrade authority mismatch: on-chain is X, your keypair is Y" (new) |
| `quasar deploy --upgrade` on immutable program | Error before buffer upload: "program is immutable (no upgrade authority)" (new) |
| Chunk write fails mid-upload | Error with buffer address for manual cleanup |
| RPC unreachable | Error: "failed to connect to RPC at <url>" |
| Transaction confirmation timeout | Error: "transaction not confirmed within N seconds" |

## Testing Strategy

**Unit tests (no RPC):**
- BPF Loader instruction serialization — all 5 variants compared against known byte patterns
- ComputeBudget SetComputeUnitPrice serialization
- Chunk calculation (file size → expected number of chunks at 950 bytes each)
- Priority fee median calculation (edge cases: empty, single value, even count, odd count)
- Programdata authority parsing (Option::Some and Option::None paths)
- Buffer header size constant validation

**Existing tests preserved:**
- All 30 existing tests continue to pass
- Squads-specific tests stay in `multisig.rs`
- RPC helper tests (cluster resolution, tilde expansion) move to `rpc.rs`
- Address constant tests (BPF loader, sysvars) move to `bpf_loader.rs`

## What's NOT In Scope

- Parallel buffer uploads (future optimization)
- Transaction retry with backoff (future, sequential is fine for v1)
- `anchor verify` style binary verification
- Buffer cleanup on failure (user does `solana program close` manually)
- Compute unit limit estimation (use defaults)
