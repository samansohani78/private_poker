# Test Coverage Baseline - Sprint 6 Start

**Date:** Sprint 6 Day 1
**Tool:** cargo-llvm-cov v0.6.21
**Overall Coverage:** 62.66% lines (1,781 of 4,770 lines missed)

---

## Summary Statistics

| Metric | Coverage | Target | Gap |
|--------|----------|--------|-----|
| **Total Lines** | 62.66% | 90% | -27.34% |
| **Total Regions** | 61.80% | 90% | -28.20% |
| **Total Functions** | 64.03% | 90% | -25.97% |

---

## Module-Level Coverage

### Core Library (private_poker)

| Module | Lines | Covered | Missed | Coverage | Target | Gap | Priority |
|--------|-------|---------|--------|----------|--------|-----|----------|
| **game.rs** (FSM) | 2,030 | 1,696 | 334 | **83.55%** | 98% | -14.45% | Critical |
| **functional.rs** | 233 | 231 | 2 | **99.14%** | 99% | +0.14% | ✅ Good |
| **entities.rs** | 253 | 132 | 121 | **52.17%** | 95% | -42.83% | High |
| **net/server.rs** | 751 | 595 | 156 | **79.23%** | 90% | -10.77% | High |
| **net/client.rs** | 109 | 63 | 46 | **57.80%** | 90% | -32.20% | High |
| **net/utils.rs** | 82 | 73 | 9 | **89.02%** | 95% | -5.98% | Medium |
| **net/messages.rs** | 31 | 0 | 31 | **0.00%** | N/A | N/A | Low* |

\* Messages module is mostly enums/structs with derived traits; low value to test directly

### Client Library (pp_client)

| Module | Lines | Covered | Missed | Coverage | Target | Gap | Priority |
|--------|-------|---------|--------|----------|--------|-----|----------|
| **commands.rs** | 202 | 199 | 3 | **98.51%** | 98% | +0.51% | ✅ Excellent |
| **app.rs** | 472 | 0 | 472 | **0.00%** | 85% | -85.00% | Medium* |
| **widgets.rs** | 104 | 0 | 104 | **0.00%** | 80% | -80.00% | Low* |

\* UI code harder to test; focus on logic extraction

### Binaries (pp_server, pp_bots)

| Module | Coverage | Notes |
|--------|----------|-------|
| pp_server/main.rs | 0% | Binary entry point; low priority |
| pp_bots/* | 0% | Bot binaries; low priority |

---

## Detailed Gap Analysis

### Critical Priority: FSM (game.rs) - 83.55% → 98%

**Missed Areas (334 lines):**

1. **Error Paths** (~100 lines)
   - Invalid action handling
   - Edge case validation
   - State transition failures

2. **Edge Case State Transitions** (~80 lines)
   - Empty player scenarios
   - Single player edge cases
   - All-in edge cases with multiple side pots

3. **Voting Logic** (~50 lines)
   - Tie-breaking scenarios
   - Concurrent votes
   - Vote cancellation

4. **Queue Management** (~50 lines)
   - Player queue edge cases
   - Simultaneous queue operations
   - Queue cleanup

5. **Pot Distribution** (~30 lines)
   - Complex side pot scenarios
   - Multiple winners with ties
   - Edge cases in split pots

6. **Other** (~24 lines)
   - Minor helper functions
   - Defensive checks
   - Edge logging

**Recommended Tests:**
- 30+ FSM edge case tests
- 20+ concurrent operation tests
- 15+ voting scenario tests
- 10+ pot distribution tests

---

### High Priority: Entities (entities.rs) - 52.17% → 95%

**Missed Areas (121 lines):**

1. **Player State Management** (~40 lines)
   - State transition validation
   - Money tracking edge cases
   - Position management

2. **Pot Management** (~30 lines)
   - Side pot creation
   - Pot distribution logic
   - Eligibility tracking

3. **Deck Operations** (~20 lines)
   - Shuffle validation
   - Deal edge cases
   - Card uniqueness

4. **User Management** (~15 lines)
   - Username validation
   - Money operations
   - User equality/hashing

5. **Card Operations** (~10 lines)
   - Card comparison
   - Suit/value validation
   - Card display

6. **Other Types** (~6 lines)
   - Blinds, Action, Vote types
   - Minor utility functions

**Recommended Tests:**
- 20+ entity creation tests
- 15+ state transition tests
- 10+ pot management tests
- 10+ deck operation tests

---

### High Priority: Network Server (net/server.rs) - 79.23% → 90%

**Missed Areas (156 lines):**

1. **Error Handling** (~60 lines)
   - Connection failures
   - Malformed messages
   - Protocol violations
   - Timeout scenarios

2. **Rate Limiting Edge Cases** (~30 lines)
   - Concurrent connection storms
   - IP tracking edge cases
   - Cleanup edge cases

3. **Token Management** (~25 lines)
   - Token recycling edge cases
   - Concurrent token operations
   - Token exhaustion

4. **Connection Lifecycle** (~20 lines)
   - Disconnect handling
   - Reconnection scenarios
   - Cleanup on errors

5. **Message Processing** (~15 lines)
   - Message queue edge cases
   - Priority handling
   - Backpressure scenarios

6. **Other** (~6 lines)
   - Helper functions
   - Defensive checks

**Recommended Tests:**
- 25+ error handling tests
- 15+ rate limiter tests
- 10+ token management tests
- 10+ connection lifecycle tests

---

### High Priority: Network Client (net/client.rs) - 57.80% → 90%

**Missed Areas (46 lines):**

1. **Connection Retry Logic** (~15 lines)
   - Timeout scenarios
   - Backoff edge cases
   - Connection failure handling

2. **Message Parsing** (~12 lines)
   - Error message handling
   - Unexpected responses
   - Malformed data

3. **Command Sending** (~10 lines)
   - Write failures
   - Partial writes
   - Connection loss during send

4. **Response Handling** (~9 lines)
   - Ack validation
   - Error propagation
   - View validation

**Recommended Tests:**
- 15+ connection failure tests
- 10+ error response tests
- 10+ message validation tests

---

### Medium Priority: Network Utils (net/utils.rs) - 89.02% → 95%

**Missed Areas (9 lines):**

1. **Edge Cases** (~5 lines)
   - Zero-length messages
   - Maximum size messages
   - Partial reads

2. **Error Paths** (~4 lines)
   - I/O errors
   - Serialization failures

**Recommended Tests:**
- 5+ edge case tests
- 3+ error handling tests

---

## Test Category Gaps

### Current Test Distribution

| Category | Current | Needed | Gap |
|----------|---------|--------|-----|
| Unit Tests | 61 | 120 | +59 |
| Integration Tests | 12 | 50 | +38 |
| Edge Case Tests | ~10 | 60 | +50 |
| Network Tests | 10 | 50 | +40 |
| Property Tests | 13 | 35 | +22 |
| Stress Tests | 0 | 10 | +10 |
| **Total** | **106** | **325** | **+219** |

### Missing Test Categories

1. **FSM Edge Cases** (0 → 50 tests)
   - Invalid state transitions
   - Boundary conditions
   - Concurrent operations
   - Complex game scenarios

2. **Network Resilience** (10 → 50 tests)
   - Connection failures
   - Malformed messages
   - Protocol violations
   - Resource exhaustion

3. **Input Validation** (0 → 30 tests)
   - Username validation
   - Amount validation
   - Command validation
   - Configuration validation

4. **Stress/Load Tests** (0 → 10 tests)
   - High concurrent users
   - Message flooding
   - Long-running games
   - Resource usage

5. **Property Tests** (13 → 35 tests)
   - FSM properties
   - Money conservation
   - Hand evaluation properties
   - Network properties

---

## Uncovered Code Patterns

### Pattern 1: Error Paths Not Tested
**Occurrences:** 150+ lines across modules

**Example from game.rs:**
```rust
pub fn remove_user(&mut self, username: &Username) -> Result<Option<bool>, UserError> {
    if !self.data.users.contains(username) {
        return Err(UserError::UserDoesNotExist); // ← Not tested
    }
    // ... rest of function tested
}
```

**Solution:** Add negative test cases for all error returns

---

### Pattern 2: Edge Case Branches
**Occurrences:** 100+ lines across modules

**Example from entities.rs:**
```rust
pub fn deal(&mut self, n: usize) -> Vec<Card> {
    if n > self.cards.len() {  // ← Edge case not tested
        panic!("not enough cards");
    }
    // ... normal path tested
}
```

**Solution:** Test boundary conditions (n=0, n=52, n=53)

---

### Pattern 3: Concurrent Operation Paths
**Occurrences:** 80+ lines in server.rs, game.rs

**Example from server.rs:**
```rust
fn handle_concurrent_connects(&mut self) {
    // Multiple paths for simultaneous operations
    // Many branches untested
}
```

**Solution:** Multi-threaded integration tests

---

### Pattern 4: Cleanup/Resource Management
**Occurrences:** 50+ lines across modules

**Example from server.rs:**
```rust
fn cleanup_client(&mut self, token: Token) {
    // Cleanup logic with multiple branches
    // Edge cases not tested
}
```

**Solution:** Resource lifecycle tests

---

## Coverage Improvement Plan

### Phase 1: Critical Gaps (Week 1)
**Target:** 62.66% → 75%

1. FSM edge cases (game.rs)
2. Entity tests (entities.rs)
3. Network error paths (server.rs, client.rs)

**Expected Gain:** ~12% coverage (+600 lines)

### Phase 2: High-Value Tests (Week 2)
**Target:** 75% → 85%

1. Network resilience tests
2. Input validation
3. Integration test expansion
4. Property test expansion

**Expected Gain:** ~10% coverage (+500 lines)

### Phase 3: Comprehensive Coverage (Week 3)
**Target:** 85% → 90%+

1. Stress tests
2. Remaining edge cases
3. UI code coverage (where feasible)
4. Documentation updates

**Expected Gain:** ~5% coverage (+250 lines)

---

## High-Value Test Targets

### Top 10 Functions to Test (by impact)

1. **`Game<T>::remove_user()`** - 15+ untested branches
2. **`Game<T>::kick_user()`** - 12+ untested error paths
3. **`Pot::distribute()`** - 10+ edge cases
4. **`Server::handle_error()`** - 8+ failure scenarios
5. **`Client::connect()`** - 7+ retry scenarios
6. **`Game<T>::vote()`** - 6+ concurrent vote cases
7. **`RateLimiter::check()`** - 5+ edge cases
8. **`Deck::deal()`** - 4+ boundary conditions
9. **`Player::transition_state()`** - 4+ invalid transitions
10. **`Utils::read_prefixed()`** - 3+ error paths

---

## Recommendations

### Immediate Actions (Stage 2-3)
1. Add FSM edge case tests (+50 tests, +10% coverage)
2. Add network resilience tests (+40 tests, +8% coverage)
3. Add entity tests (+35 tests, +7% coverage)

### Medium Term (Stage 4-6)
1. Implement stress testing framework
2. Expand property-based tests
3. Add input validation tests

### Long Term (Stage 7-8)
1. CI integration with coverage enforcement
2. Coverage badge in README
3. Regular coverage monitoring

---

## Exclusions

The following are intentionally excluded from coverage targets:

1. **Binary entry points** (main.rs files) - Hard to test meaningfully
2. **UI code** (app.rs, widgets.rs) - Requires integration testing
3. **Derived traits** (Debug, Display, etc.) - Compiler-generated
4. **Message enums** (messages.rs) - Just data structures

These account for ~800 lines (16.8% of total codebase).

**Adjusted Target:** 90% of testable code = ~75% of total codebase

---

## Success Metrics

### Coverage Targets by Stage

| Stage | Module | Before | Target | Tests Added |
|-------|--------|--------|--------|-------------|
| 2 | game.rs | 83.55% | 95% | +50 |
| 2 | entities.rs | 52.17% | 85% | +35 |
| 3 | server.rs | 79.23% | 90% | +25 |
| 3 | client.rs | 57.80% | 85% | +15 |
| 3 | utils.rs | 89.02% | 95% | +8 |
| 4-6 | Integration | - | - | +65 |

**Overall Target:** 62.66% → 90% coverage
**Tests Added:** 121 → 325+ tests (+204)

---

## HTML Coverage Report

Full detailed coverage report available at:
`target/llvm-cov/html/index.html`

**Key Files to Review:**
- `private_poker/src/game.rs` - FSM implementation
- `private_poker/src/game/entities.rs` - Core types
- `private_poker/src/net/server.rs` - Server networking
- `private_poker/src/net/client.rs` - Client networking

---

## Next Steps

1. ✅ **Stage 1 Complete:** Coverage baseline established
2. **Stage 2:** Add FSM edge case tests (game.rs, entities.rs)
3. **Stage 3:** Add network resilience tests (server.rs, client.rs)
4. **Stage 4:** Implement stress testing framework
5. **Stage 5:** Add input validation tests
6. **Stage 6:** Expand property-based tests
7. **Stage 7:** Expand integration tests
8. **Stage 8:** Documentation & CI integration

---

**Report Generated:** Sprint 6 Day 1
**Tool Version:** cargo-llvm-cov v0.6.21
**Command:** `cargo llvm-cov --workspace --html`
