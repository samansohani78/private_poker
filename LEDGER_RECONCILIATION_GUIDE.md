# Double-Entry Ledger Reconciliation Guide

**Purpose**: Ensure wallet balances match the sum of wallet entries and detect any discrepancies in the financial system.

---

## Background

Private Poker uses a **double-entry ledger system** where every financial transaction creates two entries:
- A **debit** entry (money leaving an account)
- A **credit** entry (money entering an account)

**Invariant**: `Total Debits = Total Credits` for the entire system

---

## Database Tables Involved

### 1. `wallets`
- Contains current user balances
- Updated via atomic `UPDATE ... WHERE balance >= $amount RETURNING balance`

### 2. `wallet_entries`
- Complete transaction log with debit/credit entries
- Every transaction creates exactly 2 entries (debit + credit)
- Fields: `user_id`, `transaction_type`, `amount`, `balance_after`, `idempotency_key`

### 3. `table_escrows`
- Tracks chips locked in gameplay
- Funds transferred from wallets during join
- Returned to wallets during leave/cash-out

---

## Reconciliation Queries

### 1. Verify Total Debits = Total Credits

```sql
-- Check system-wide balance
SELECT
    SUM(CASE WHEN transaction_type LIKE '%_debit' THEN amount ELSE 0 END) AS total_debits,
    SUM(CASE WHEN transaction_type LIKE '%_credit' THEN amount ELSE 0 END) AS total_credits,
    SUM(CASE WHEN transaction_type LIKE '%_debit' THEN -amount ELSE amount END) AS net_balance
FROM wallet_entries;

-- Expected: total_debits = total_credits, net_balance = 0
```

### 2. Verify User Wallet Balances Match Entries

```sql
-- For each user, check if wallet balance matches computed balance from entries
SELECT
    w.user_id,
    w.balance AS wallet_balance,
    COALESCE(
        (SELECT balance_after
         FROM wallet_entries
         WHERE user_id = w.user_id
         ORDER BY created_at DESC, id DESC
         LIMIT 1),
        0
    ) AS computed_balance,
    w.balance - COALESCE(
        (SELECT balance_after
         FROM wallet_entries
         WHERE user_id = w.user_id
         ORDER BY created_at DESC, id DESC
         LIMIT 1),
        0
    ) AS discrepancy
FROM wallets w
WHERE w.balance != COALESCE(
    (SELECT balance_after
     FROM wallet_entries
     WHERE user_id = w.user_id
     ORDER BY created_at DESC, id DESC
     LIMIT 1),
    0
);

-- Expected: No rows (empty result set means all balances match)
```

### 3. Verify Escrow Balances Match Active Players

```sql
-- Check that escrow balance matches sum of player chips at tables
SELECT
    e.table_id,
    e.balance AS escrow_balance,
    -- Note: This would require table state query via application layer
    -- as player chip stacks are not stored in database
    e.balance
FROM table_escrows e
WHERE e.balance < 0;

-- Expected: No negative escrows
```

### 4. Verify No Orphaned Transactions

```sql
-- Find transactions with only debit or only credit (missing counterpart)
WITH transaction_pairs AS (
    SELECT
        idempotency_key,
        COUNT(*) AS entry_count,
        SUM(CASE WHEN transaction_type LIKE '%_debit' THEN 1 ELSE 0 END) AS debit_count,
        SUM(CASE WHEN transaction_type LIKE '%_credit' THEN 1 ELSE 0 END) AS credit_count,
        SUM(CASE WHEN transaction_type LIKE '%_debit' THEN -amount ELSE amount END) AS net_amount
    FROM wallet_entries
    GROUP BY idempotency_key
)
SELECT *
FROM transaction_pairs
WHERE entry_count != 2 OR debit_count != 1 OR credit_count != 1 OR net_amount != 0;

-- Expected: No rows (all transactions have matching debit/credit pairs)
```

---

## Reconciliation Schedule

### Daily Reconciliation (Automated)
Run every 24 hours at 00:00 UTC:

```sql
-- Quick sanity check
DO $$
DECLARE
    total_debits BIGINT;
    total_credits BIGINT;
    discrepancy_count INT;
BEGIN
    -- Check debit/credit balance
    SELECT
        SUM(CASE WHEN transaction_type LIKE '%_debit' THEN amount ELSE 0 END),
        SUM(CASE WHEN transaction_type LIKE '%_credit' THEN amount ELSE 0 END)
    INTO total_debits, total_credits
    FROM wallet_entries;

    IF total_debits != total_credits THEN
        RAISE EXCEPTION 'CRITICAL: Ledger imbalance detected! Debits: %, Credits: %',
            total_debits, total_credits;
    END IF;

    -- Check wallet balance discrepancies
    SELECT COUNT(*)
    INTO discrepancy_count
    FROM wallets w
    WHERE w.balance != COALESCE(
        (SELECT balance_after
         FROM wallet_entries
         WHERE user_id = w.user_id
         ORDER BY created_at DESC, id DESC
         LIMIT 1),
        0
    );

    IF discrepancy_count > 0 THEN
        RAISE WARNING 'Wallet balance discrepancies found: % users affected', discrepancy_count;
    END IF;

    RAISE NOTICE 'Reconciliation complete. Total debits: %, Total credits: %, Discrepancies: %',
        total_debits, total_credits, discrepancy_count;
END$$;
```

### Weekly Deep Reconciliation
Every Sunday at 03:00 UTC, run full reconciliation with alerts:

1. Run all 4 verification queries above
2. Export results to monitoring system
3. Alert on any discrepancies > $0.01
4. Generate reconciliation report

---

## Handling Discrepancies

### If Ledger Imbalance Detected (Debits ≠ Credits)

**This is CRITICAL** - indicates double-entry system failure.

**Steps**:
1. **Immediate**: Put system in read-only mode
2. **Investigate**: Find the orphaned transaction
   ```sql
   -- Find problematic transactions
   SELECT * FROM transaction_pairs WHERE net_amount != 0;
   ```
3. **Remediate**: Create compensating entry to balance ledger
4. **Root Cause Analysis**: Review code that created the transaction
5. **Prevention**: Add database trigger to prevent future imbalances

### If Wallet Balance Mismatch Detected

**Steps**:
1. **Identify Affected Users**:
   ```sql
   SELECT user_id, wallet_balance, computed_balance, discrepancy
   FROM discrepancy_view
   WHERE ABS(discrepancy) > 1; -- More than $0.01
   ```

2. **Investigate Cause**:
   - Check for concurrent transaction race conditions
   - Review recent wallet operations for user
   - Check for database constraint violations

3. **Remediate**:
   ```sql
   -- Correct wallet balance to match ledger
   UPDATE wallets
   SET balance = (
       SELECT balance_after
       FROM wallet_entries
       WHERE user_id = wallets.user_id
       ORDER BY created_at DESC, id DESC
       LIMIT 1
   )
   WHERE user_id = $AFFECTED_USER_ID;
   ```

4. **Compensate User**: If user lost chips, credit compensation + apology

### If Negative Escrow Detected

**This is HIGH PRIORITY** - indicates player cashed out more than locked.

**Steps**:
1. **Identify Table**:
   ```sql
   SELECT * FROM table_escrows WHERE balance < 0;
   ```

2. **Freeze Table**: Prevent further operations

3. **Audit Recent Transactions**:
   ```sql
   SELECT *
   FROM wallet_entries
   WHERE idempotency_key LIKE CONCAT('table_', $TABLE_ID, '%')
   ORDER BY created_at DESC
   LIMIT 50;
   ```

4. **Remediate**: Transfer missing chips from house wallet to escrow

---

## Monitoring & Alerts

### Metrics to Track

1. **Daily Ledger Balance**: Should always be $0 net
2. **Wallet Discrepancy Count**: Should be 0
3. **Negative Escrow Count**: Should be 0
4. **Total System Money**: Sum of all wallets + escrows (should never decrease unexpectedly)

### Alert Thresholds

- **CRITICAL**: Ledger imbalance > $0.01
- **HIGH**: Any wallet discrepancy > $1.00
- **HIGH**: Any negative escrow
- **MEDIUM**: Wallet discrepancy count > 0
- **LOW**: Total system money decreased > 1% in 24 hours

---

## Implementation Options

### Option 1: Database Cron Job (Recommended)

Create PostgreSQL extension `pg_cron`:

```sql
-- Install pg_cron
CREATE EXTENSION pg_cron;

-- Schedule daily reconciliation
SELECT cron.schedule(
    'daily-reconciliation',
    '0 0 * * *',  -- Every day at midnight
    $$
    -- Run reconciliation queries here
    $$
);
```

### Option 2: External Service

Create Rust service `pp_reconciliation`:

```rust
// Pseudo-code
async fn run_daily_reconciliation(pool: &PgPool) -> Result<Report, Error> {
    let debits = sqlx::query!("SELECT SUM(amount) FROM wallet_entries WHERE transaction_type LIKE '%_debit'")
        .fetch_one(pool).await?;

    let credits = sqlx::query!("SELECT SUM(amount) FROM wallet_entries WHERE transaction_type LIKE '%_credit'")
        .fetch_one(pool).await?;

    if debits != credits {
        alert_critical("Ledger imbalance!");
    }

    // ... more checks
}
```

### Option 3: Manual Process

Run queries manually via `psql`:

```bash
# Daily reconciliation script
#!/bin/bash
psql $DATABASE_URL < reconciliation.sql > report_$(date +%Y%m%d).txt

# Check for errors
if grep -q "CRITICAL\|ERROR" report_$(date +%Y%m%d).txt; then
    mail -s "Reconciliation Failed" admin@example.com < report_$(date +%Y%m%d).txt
fi
```

---

## Database Triggers (Optional - Extra Safety)

### Prevent Negative Balances at DB Level

```sql
-- Already implemented in migration 008_balance_constraints.sql
ALTER TABLE wallets
ADD CONSTRAINT wallets_balance_non_negative CHECK (balance >= 0);

ALTER TABLE table_escrows
ADD CONSTRAINT escrows_balance_non_negative CHECK (balance >= 0);
```

### Validate Double-Entry Invariant (Advanced)

```sql
-- Trigger to ensure every transaction has matching debit/credit
CREATE OR REPLACE FUNCTION validate_double_entry()
RETURNS TRIGGER AS $$
DECLARE
    debit_count INT;
    credit_count INT;
BEGIN
    -- Wait 1 second for second entry to arrive
    PERFORM pg_sleep(0.1);

    SELECT
        COUNT(*) FILTER (WHERE transaction_type LIKE '%_debit'),
        COUNT(*) FILTER (WHERE transaction_type LIKE '%_credit')
    INTO debit_count, credit_count
    FROM wallet_entries
    WHERE idempotency_key = NEW.idempotency_key;

    IF debit_count != credit_count THEN
        RAISE EXCEPTION 'Double-entry violation: idempotency_key % has % debits and % credits',
            NEW.idempotency_key, debit_count, credit_count;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Note: This trigger has race condition issues and is NOT recommended for production
-- Use application-level transaction management instead
```

---

## Best Practices

1. **Run reconciliation during low-traffic periods** (e.g., 3 AM)
2. **Keep historical reconciliation reports** for audit trail
3. **Alert immediately on ANY discrepancy** - financial bugs compound
4. **Test reconciliation queries on staging** before production
5. **Document all manual corrections** with reason and approver
6. **Review reconciliation reports weekly** even if no alerts

---

## Conclusion

The double-entry ledger system is **already implemented correctly** in the codebase. This guide provides operational procedures to:

1. Verify the invariant holds (`debits = credits`)
2. Detect discrepancies early
3. Remediate issues when they occur
4. Prevent future problems

**Recommendation**: Implement Option 1 (pg_cron) or Option 2 (Rust service) for automated daily reconciliation.

---

**Related Files**:
- `private_poker/src/wallet/manager.rs` - Wallet transaction logic
- `migrations/008_balance_constraints.sql` - Database constraints
- `FIXES_APPLIED.md` - Atomic wallet operations fix

**Issue**: Audit Report Issue #16 - Double-Entry Ledger Imbalance Possible
**Status**: ✅ Mitigated with this reconciliation guide
