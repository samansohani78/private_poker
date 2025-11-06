# Testing Guide

## Overview

This project maintains comprehensive test coverage across unit tests, integration tests, property-based tests, and stress tests. Our testing philosophy prioritizes meaningful coverage over raw metrics, with a focus on edge cases, failure modes, and system resilience.

## Test Categories

### 1. Unit Tests

Located within source files using `#[cfg(test)]` modules. Unit tests verify individual components in isolation.

**Run unit tests:**
```bash
cargo test -p private_poker --lib
cargo test -p pp_client --lib
```

**Coverage areas:**
- Game entities (Cards, Decks, Players, Pots, Actions)
- Hand evaluation logic
- FSM state transitions
- Network message serialization
- Input validation

### 2. Integration Tests

Located in `private_poker/tests/`. Integration tests verify end-to-end behavior.

**Run integration tests:**
```bash
cargo test -p private_poker --test client_server
cargo test -p private_poker --test game_flow_integration
```

**Coverage areas:**
- Client-server communication
- Complete game flows (lobby → deal → betting → showdown)
- Multi-player scenarios
- Error handling and recovery

### 3. Property-Based Tests

Located in `private_poker/tests/hand_evaluation_proptest.rs`. Property tests verify invariants across thousands of random inputs.

**Run property tests:**
```bash
cargo test -p private_poker --test hand_evaluation_proptest
```

**Coverage areas:**
- Hand evaluation correctness
- Comparison transitivity
- Deterministic evaluation
- Winner selection validity

### 4. Stress Tests

Embedded in unit tests with `stress_test_*` prefix. Stress tests verify system behavior under high load.

**Run stress tests:**
```bash
cargo test stress_test
```

**Coverage areas:**
- High-volume message processing (500-1000 operations)
- Large data structures (500KB+ payloads, 10K+ element vectors)
- Deep game progression (20+ hands, 100+ games)
- Performance stability over time

## Running Tests

### Quick Test
```bash
# Run all tests
cargo test --all

# Run tests for specific package
cargo test -p private_poker
cargo test -p pp_client
```

### With Coverage
```bash
# Generate coverage report
cargo llvm-cov --all --html

# View coverage in browser
open target/llvm-cov/html/index.html
```

### Continuous Integration
```bash
# CI-friendly test run (treats warnings as errors)
cargo clippy --all -- -D warnings
cargo test --all
```

## Writing Good Tests

### Test Naming Conventions

- Unit tests: `test_<component>_<scenario>`
  - Example: `test_username_empty_string`
- Property tests: `test_<property>_<condition>`
  - Example: `test_hand_comparison_transitive`
- Stress tests: `stress_test_<operation>_<scale>`
  - Example: `stress_test_many_sequential_messages`
- Integration tests: `test_<feature>_<flow>`
  - Example: `test_full_game_flow_with_all_in`

### Test Structure

```rust
#[test]
fn test_descriptive_name() {
    // Arrange: Set up test data
    let mut state = PokerState::new();
    let user = Username::new("test");

    // Act: Perform the operation
    let result = state.new_user(&user);

    // Assert: Verify the outcome
    assert!(result.is_ok());
    assert_eq!(state.get_views().len(), 1);
}
```

### Edge Cases to Test

1. **Boundary values**: Empty, zero, max values
2. **Invalid inputs**: Malformed data, out-of-range values
3. **State transitions**: Valid and invalid state changes
4. **Concurrency**: Race conditions, simultaneous operations
5. **Resource limits**: Memory exhaustion, connection limits
6. **Error paths**: Network failures, timeouts, disconnections

### Property Test Guidelines

Use property-based tests when testing:
- Mathematical properties (commutativity, associativity, identity)
- Invariants (money conservation, no negative balances)
- Round-trip serialization
- Comparison relationships (transitivity, reflexivity)

```rust
proptest! {
    #[test]
    fn test_property_name(input in strategy()) {
        // Verify property holds for all inputs
        prop_assert!(property_check(input));
    }
}
```

## Coverage Requirements

### Minimum Coverage Targets

| Component | Target | Current |
|-----------|--------|---------|
| Core Library | 90% | ~77% |
| Game Logic | 95% | ~90% |
| Network Layer | 85% | ~95% |
| Entities | 95% | ~98% |

### Coverage Philosophy

We prioritize:
1. **Edge case coverage** over happy path duplication
2. **Failure mode testing** over success scenarios
3. **Property verification** over example-based tests
4. **Integration coverage** over isolated unit tests

## Test Organization

```
private_poker/
├── src/
│   ├── game.rs          # Game FSM tests
│   ├── game/
│   │   ├── entities.rs  # Entity tests (username, actions, etc.)
│   │   └── functional.rs # Hand evaluation tests
│   └── net/
│       ├── messages.rs  # Message serialization tests
│       ├── utils.rs     # Network utility tests
│       └── server.rs    # Server tests
└── tests/
    ├── client_server.rs # Client-server integration
    ├── game_flow_integration.rs # Game flow scenarios
    └── hand_evaluation_proptest.rs # Property tests
```

## Common Test Patterns

### Testing State Machines
```rust
let mut state = PokerState::new();
state = state.step(); // Advance state
assert!(matches!(state, PokerState::Deal(_)));
```

### Testing Serialization
```rust
let original = ClientMessage { /* ... */ };
let serialized = bincode::serialize(&original).unwrap();
let deserialized = bincode::deserialize(&serialized).unwrap();
assert_eq!(original, deserialized);
```

### Testing Error Paths
```rust
let result = state.take_action(&user, Action::Raise(Some(u32::MAX)));
assert!(result.is_err());
```

## Debugging Test Failures

### Enable Logging
```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Run Single Test
```bash
cargo test test_specific_name -- --exact
```

### Show Test Output
```bash
cargo test -- --nocapture --test-threads=1
```

## Continuous Improvement

- Review coverage reports monthly
- Add tests for every bug fix
- Update property tests when adding features
- Maintain fast test suite (< 5s for unit tests)
- Keep integration tests independent and parallelizable

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Proptest Book](https://altsysrq/proptest-book)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
