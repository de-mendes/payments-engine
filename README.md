# Payments Engine

A simple payments engine that processes transactions from a CSV file, handles deposits, withdrawals, disputes, resolutions, and chargebacks, then outputs client account states.

## Usage
```bash
cargo build --release
cargo run -- transactions.csv > accounts.csv
```

## Expected Input Format
CSV with columns: `type`, `client`, `tx`, `amount`

```csv
type, client, tx, amount
deposit, 1, 1, 100.0
withdrawal, 1, 2, 50.0
dispute, 1, 1,
resolve, 1, 1,
```

- Whitespace around values is trimmed
- Precision up to 4 decimal places

## Output Format
CSV with columns: `client`, `available`, `held`, `total`, `locked`

```csv
client,available,held,total,locked
1,50,0,50,false
```

## Transaction Types
| Type         | Description                                                                 |
|--------------|-----------------------------------------------------------------------------|
| `deposit`    | Credits client account (available + total increase)                         |
| `withdrawal` | Debits client account if sufficient available funds                         |
| `dispute`    | Holds funds from a referenced deposit (available decreases, held increases) |
| `resolve`    | Releases held funds back to available                                       |
| `chargeback` | Reverses disputed transaction, locks account                                |

## Design Decisions

### State Machine for Disputes

Transactions use the following state machine:

```
Deposit <—————|
   |          |
Dispute —> Resolve
   |        
ChargeBack
```

- Only deposits can be disputed
- A transaction must be disputed before it can be resolved or charged back
- Once resolved or charged back, a transaction cannot be disputed again

### Error Handling
- Invalid transactions are logged to stderr and skipped
- Processing continues even if individual transactions fail
- Fatal errors (file not found, CSV write failures) exit with code 1

### Memory Management
- CSV records are streamed and processed one at a time
- Only deposit transactions are stored (required for dispute lookups)
- Charged back transactions are removed from memory

### Locked Accounts
- An account is locked after a chargeback occurs
- Locked accounts reject all further transactions (deposits, withdrawals, disputes)

## Rules
1. **Transaction IDs are globally unique** - A deposit and withdrawal cannot share the same tx ID
2. **Only deposits can be disputed** - Withdrawals are not stored and cannot be referenced by disputes
3. **Disputes require sufficient available funds** - If a client has already withdrawn the disputed amount, the dispute still succeeds but only moves what's available to held
4. **Client ID in dispute must match original transaction** - A client cannot dispute another client's transaction
5. **Withdrawals on non-existent accounts are no-ops** - No error, but no account is created
6. **Charged back transactions are removed** - They cannot be disputed again
7. **After a resolution. A deposit can be disputed again** - Deposits may only have a single open dispute, but may be disputed several times.

## Testing
Run unit tests:

```bash
cargo test
```

Tests cover:
- State transition validation (valid and invalid paths)
- Account lock checking
- Transaction transition handling (not found, wrong client, invalid state, success)

Run with sample data:
```bash
cargo build --release
cargo run -- transactions.csv > accounts.csv
```
## Dependencies
- `csv` - CSV parsing and writing
- `rust_decimal` - Precise decimal arithmetic for financial calculations
- `serde` - Serialization/deserialization

## Rust Version

Requires Rust `1.85+` (edition `2024`).
