# Coding Guidelines

This document establishes coding standards and best practices for the Private Poker project, distilled from 6 sprints of development and refinement.

---

## Table of Contents

1. [General Principles](#general-principles)
2. [Function Design](#function-design)
3. [Module Organization](#module-organization)
4. [Error Handling](#error-handling)
5. [Testing Requirements](#testing-requirements)
6. [Documentation Standards](#documentation-standards)
7. [Performance Considerations](#performance-considerations)
8. [Common Patterns](#common-patterns)
9. [Code Review Checklist](#code-review-checklist)

---

## General Principles

### Clarity Over Cleverness

**Good:**
```rust
fn is_player_active(player: &Player) -> bool {
    matches!(player.state, PlayerState::Check | PlayerState::Call | PlayerState::Raise)
}
```

**Bad:**
```rust
fn is_player_active(player: &Player) -> bool {
    [PlayerState::Check, PlayerState::Call, PlayerState::Raise]
        .contains(&player.state)
}
```

- Code should be self-documenting where possible
- Prefer explicit over implicit
- Use descriptive names that reveal intent
- Avoid unnecessary abstractions

### Leverage the Type System

**Good:**
```rust
pub struct Username(String);  // Type wrapper prevents misuse

impl Username {
    pub fn new(s: &str) -> Self {
        let sanitized = sanitize_username(s);
        Self(sanitized)
    }
}
```

**Bad:**
```rust
pub type Username = String;  // No validation or type safety
```

- Use newtypes for semantic clarity
- Make invalid states unrepresentable
- Prefer compile-time errors over runtime checks
- Use zero-sized types for state markers

### Functional Programming Principles

- Prefer pure functions where possible
- Use iterators over manual loops
- Avoid unnecessary mutation
- Return results instead of side effects

---

## Function Design

### Size Limits

**Enforced Maximum**: 80 lines per function
**Target Range**: 20-40 lines for most functions
**Ideal**: < 20 lines

**When a function exceeds 40 lines:**
1. Look for logical sub-components
2. Extract helper functions
3. Use composition over complexity

**Example Refactoring:**

```rust
// Before: 235-line monolithic function
fn draw(&mut self, view: &GameView, frame: &mut Frame) {
    // 235 lines of rendering logic...
}

// After: Orchestration with focused helpers
fn draw(&mut self, view: &GameView, frame: &mut Frame) {
    let layout = self.create_layout(frame.size());

    self.draw_spectators(view, frame, layout.spectators);
    self.draw_waitlist(view, frame, layout.waitlist);
    self.draw_table(view, frame, layout.table);
    self.draw_board(view, frame, layout.board);
    self.draw_actions(view, frame, layout.actions);
}

// Each helper is 10-20 focused lines
fn draw_spectators(&self, view: &GameView, frame: &mut Frame, area: Rect) {
    let spectators = view.spectators.iter()
        .map(|user| format!("{}: ${}", user.name, user.money))
        .collect::<Vec<_>>();

    let widget = List::new(spectators).block(Block::default().title("Spectators"));
    frame.render_widget(widget, area);
}
```

### Single Responsibility

Each function should have one clear purpose:

**Good:**
```rust
fn validate_raise_amount(amount: Option<Usd>, current_bet: Usd, stack: Usd) -> Result<Usd, ActionError> {
    // Single purpose: validate raise amount
    match amount {
        Some(amt) if amt < current_bet => Err(ActionError::InsufficientRaise),
        Some(amt) if amt > stack => Err(ActionError::InsufficientFunds),
        Some(amt) => Ok(amt),
        None => Ok(stack), // All-in
    }
}
```

**Bad:**
```rust
fn handle_raise(state: &mut GameState, player: &Player, amount: Option<Usd>) -> Result<(), ActionError> {
    // Multiple responsibilities: validation, state updates, event generation
    // Violates single responsibility principle
}
```

### Parameter Guidelines

- **Max parameters**: 5 (prefer 3 or fewer)
- **Use structs** for related parameters
- **Avoid boolean flags** - use enums instead

**Good:**
```rust
struct PlayerConfig {
    username: Username,
    buy_in: Usd,
    seat: usize,
}

fn create_player(config: PlayerConfig) -> Player {
    // ...
}
```

**Bad:**
```rust
fn create_player(username: Username, buy_in: Usd, seat: usize, is_dealer: bool, is_small_blind: bool) -> Player {
    // Too many parameters
}
```

---

## Module Organization

### File Structure

**Good:**
```
pp_client/src/
├── main.rs          # Minimal - just calls run()
├── app.rs           # Core app state (~400 lines)
├── commands.rs      # Command parsing (~150 lines)
├── rendering.rs     # UI rendering helpers (~300 lines)
└── widgets/         # Custom widgets
    ├── table.rs
    └── cards.rs
```

**Bad:**
```
pp_client/src/
├── main.rs
└── app.rs          # Everything in one file (1000+ lines)
```

### Module Boundaries

- **Separate concerns**: UI, business logic, networking
- **Public API minimal**: Only expose what's necessary
- **Private by default**: Make things public only when needed
- **Module documentation**: Every public module needs `//!` docs

### Import Organization

```rust
// Standard library
use std::collections::HashMap;
use std::sync::Arc;

// External crates
use serde::{Deserialize, Serialize};
use enum_dispatch::enum_dispatch;

// Internal crate modules
use crate::game::entities::{Card, Player, Username};
use crate::game::functional::eval;

// Parent modules
use super::GameData;
```

---

## Error Handling

### Use Result Types

**Prefer Result over panics** for recoverable errors:

```rust
// Good
fn parse_raise_amount(input: &str) -> Result<Usd, ParseError> {
    input.parse::<Usd>()
        .map_err(|_| ParseError::InvalidAmount(input.to_string()))
}

// Bad
fn parse_raise_amount(input: &str) -> Usd {
    input.parse::<Usd>().expect("Failed to parse amount")
}
```

### Descriptive Error Messages

```rust
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error("insufficient raise: need at least ${min}, got ${got}")]
    InsufficientRaise { min: Usd, got: Usd },

    #[error("not your turn (current player: {current})")]
    NotYourTurn { current: Username },

    #[error("invalid action in state {state:?}")]
    InvalidState { state: String },
}
```

### Error Context

Provide context when propagating errors:

```rust
// Good
fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let contents = fs::read_to_string(path)
        .map_err(|e| ConfigError::ReadFailed {
            path: path.to_owned(),
            source: e,
        })?;

    toml::from_str(&contents)
        .map_err(|e| ConfigError::ParseFailed {
            path: path.to_owned(),
            source: e,
        })
}
```

---

## Testing Requirements

### Coverage Requirements

- **New code**: 90%+ coverage required
- **Public APIs**: 100% coverage mandatory
- **Critical paths**: 100% coverage (FSM, hand evaluation, networking)

### Test Naming

```rust
// Unit tests
#[test]
fn test_username_empty_string() { }

#[test]
fn test_username_exceeds_max_length() { }

// Property tests
#[test]
fn property_hand_comparison_transitive() { }

// Integration tests
#[test]
fn test_full_game_flow_with_all_in() { }

// Stress tests
#[test]
fn stress_test_many_sequential_messages() { }
```

### Test Structure

```rust
#[test]
fn test_descriptive_name() {
    // Arrange: Set up test data
    let mut state = PokerState::new();
    let user = Username::new("alice");
    state.new_user(&user).unwrap();

    // Act: Perform operation
    let result = state.take_action(&user, Action::Fold);

    // Assert: Verify outcome
    assert!(result.is_ok());
    assert_eq!(state.get_views().len(), 1);
}
```

### Test Coverage by Type

**Unit Tests**: Individual functions and methods
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_username_sanitization() {
        let username = Username::new("user name");
        assert_eq!(username.to_string(), "user_name");
    }
}
```

**Integration Tests**: End-to-end flows
```rust
#[test]
fn test_full_game_with_multiple_players() {
    let mut state = PokerState::new();

    // Add players
    for i in 0..5 {
        state.new_user(&Username::new(&i.to_string())).unwrap();
    }

    // Play through a hand
    state.init_start(&Username::new("0")).unwrap();
    // ... complete game flow
}
```

**Property Tests**: Invariant verification
```rust
proptest! {
    #[test]
    fn property_money_conservation(players in 2..=10) {
        let state = init_game_with_players(players);
        let initial_total = calculate_total_money(&state);

        // Play hand
        let final_state = play_one_hand(state);
        let final_total = calculate_total_money(&final_state);

        prop_assert_eq!(initial_total, final_total);
    }
}
```

### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === Username Tests ===

    #[test]
    fn test_username_empty() { }

    #[test]
    fn test_username_long() { }

    // === Action Tests ===

    #[test]
    fn test_action_fold() { }

    #[test]
    fn test_action_raise() { }

    // === Edge Cases ===

    #[test]
    fn test_edge_case_all_players_all_in() { }
}
```

---

## Documentation Standards

### Public API Documentation

**Required for all public items:**

```rust
/// Parse a command string into a UserCommand.
///
/// This function handles all user input from the TUI, including
/// whitespace normalization and case-insensitive parsing.
///
/// # Arguments
///
/// * `input` - The raw command string from user input
///
/// # Returns
///
/// * `Ok(UserCommand)` - Successfully parsed command
/// * `Err(ParseError)` - Parse error with descriptive message
///
/// # Examples
///
/// ```
/// use pp_client::commands::parse_command;
///
/// let cmd = parse_command("call").unwrap();
/// assert_eq!(cmd, UserCommand::TakeAction(Action::Call));
/// ```
///
/// # Errors
///
/// Returns `ParseError::UnrecognizedCommand` if the input doesn't
/// match any known command pattern.
pub fn parse_command(input: &str) -> Result<UserCommand, ParseError> {
    // ...
}
```

### Module Documentation

```rust
//! Command parsing for the poker client.
//!
//! This module provides command parsing logic extracted from the main
//! application loop. It handles all user input, including:
//!
//! - Action commands (fold, call, raise)
//! - Vote commands (kick, reset)
//! - State commands (waitlist, spectate, leave)
//!
//! # Examples
//!
//! ```
//! use pp_client::commands::parse_command;
//!
//! let cmd = parse_command("raise 100").unwrap();
//! ```
```

### Inline Comments

**Use sparingly** - code should be self-documenting:

```rust
// Good: Comment explains WHY, not WHAT
// We use Arc here to avoid cloning the board for each player view
let board = Arc::new(self.board.clone());

// Bad: Comment explains WHAT the code does (obvious)
// Clone the board
let board = self.board.clone();
```

---

## Performance Considerations

### Premature Optimization

> "Premature optimization is the root of all evil" - Donald Knuth

**Guidelines:**
1. **Write clear code first**
2. **Profile before optimizing** (use `cargo bench`)
3. **Optimize hot paths only** (identified by profiling)
4. **Measure impact** (benchmark before and after)

### When to Optimize

Optimize when:
- **Profiling shows** it's a bottleneck
- **Benchmark shows** significant improvement (>5%)
- **Complexity doesn't increase** significantly

Don't optimize when:
- Code is not in a hot path
- Improvement is marginal (<5%)
- Readability suffers significantly

### Common Optimizations

**Use Arc for shared read-only data:**
```rust
// Good: Arc clones are cheap (pointer increment)
pub struct GameView {
    pub blinds: Arc<Blinds>,
    pub board: Arc<Vec<Card>>,
    // ...
}

// Bad: Full clones for every view
pub struct GameView {
    pub blinds: Blinds,
    pub board: Vec<Card>,
    // ...
}
```

**Prefer iterators over manual loops:**
```rust
// Good: Compiler can optimize iterator chains
let total: u32 = players.iter()
    .map(|p| p.stack)
    .sum();

// Less optimal: Manual loop
let mut total = 0;
for player in &players {
    total += player.stack;
}
```

**Avoid unnecessary allocations:**
```rust
// Good: Reuse vector capacity
fn process_events(&mut self, events: &[Event]) {
    self.buffer.clear();  // Reuse allocation
    for event in events {
        self.buffer.push(process(event));
    }
}

// Bad: Allocate new vector each time
fn process_events(&mut self, events: &[Event]) {
    let mut buffer = Vec::new();  // New allocation
    // ...
}
```

---

## Common Patterns

### State Machine Pattern

**Zero-sized type markers for type-safe states:**

```rust
// Define state markers (zero-sized types)
pub struct Lobby;
pub struct Deal;
pub struct TakeAction;

// State-specific implementations
pub struct Game<T> {
    data: GameData,
    state: T,  // Zero-sized, no runtime cost
}

impl Game<Lobby> {
    pub fn start_game(self) -> Game<Deal> {
        // Type-safe transition
        Game {
            data: self.data,
            state: Deal,
        }
    }
}

impl Game<Deal> {
    pub fn deal_cards(mut self) -> Game<TakeAction> {
        // Transition after dealing
        self.data.deck.deal_to_players(&mut self.data.players);
        Game {
            data: self.data,
            state: TakeAction,
        }
    }
}
```

### User Management Pattern

**Unified macro for immediate vs queued operations:**

```rust
// Immediate: Non-gameplay phases
impl_user_managers!(immediate:
    Game<Lobby>,
    Game<RemovePlayers>,
);

// Queued: Gameplay phases (don't disrupt hand)
impl_user_managers!(queued:
    Game<Deal>,
    Game<TakeAction>,
    Game<Flop>,
    Game<Turn>,
    Game<River>,
);
```

### View Generation Pattern

**Arc-based sharing for common data:**

```rust
pub fn get_views(&self) -> HashMap<Username, GameView> {
    // Create shared data once
    let shared = SharedViewData {
        blinds: Arc::new(self.blinds.clone()),
        board: Arc::new(self.board.clone()),
        pot: Arc::new(self.pot.clone()),
        spectators: Arc::new(self.spectators.clone()),
        waitlist: Arc::new(self.waitlist.clone()),
    };

    // Generate personalized views with shared data
    self.users.iter().map(|username| {
        let view = GameView {
            blinds: shared.blinds.clone(),      // Cheap Arc clone
            board: shared.board.clone(),
            pot: shared.pot.clone(),
            spectators: shared.spectators.clone(),
            waitlist: shared.waitlist.clone(),
            players: personalize_players(username, &self.players),  // Personalized
        };
        (username.clone(), view)
    }).collect()
}
```

### Error Handling Pattern

**Use thiserror for ergonomic error types:**

```rust
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("user not found: {username}")]
    UserNotFound { username: String },

    #[error("game is full (capacity: {capacity})")]
    CapacityReached { capacity: usize },

    #[error("invalid action: {reason}")]
    InvalidAction { reason: String },
}

// Usage
fn add_user(&mut self, username: Username) -> Result<(), GameError> {
    if self.users.len() >= self.capacity {
        return Err(GameError::CapacityReached {
            capacity: self.capacity,
        });
    }

    self.users.insert(username);
    Ok(())
}
```

---

## Code Review Checklist

### Before Submitting PR

- [ ] **All tests pass** (`cargo test --all`)
- [ ] **No clippy warnings** (`cargo clippy --all -- -D warnings`)
- [ ] **Formatted** (`cargo fmt --all`)
- [ ] **No functions > 80 lines**
- [ ] **New code has tests** (90%+ coverage)
- [ ] **Public APIs documented** (rustdoc)
- [ ] **Benchmarks run** (if performance-critical)
- [ ] **CLAUDE.md updated** (if architecture changes)

### Reviewing Code

**Function Design:**
- [ ] Functions are < 80 lines (ideally 20-40)
- [ ] Single responsibility per function
- [ ] Clear, descriptive names
- [ ] Minimal parameters (< 5)

**Module Organization:**
- [ ] Clear separation of concerns
- [ ] Minimal public API surface
- [ ] Module documentation present
- [ ] Logical file structure

**Error Handling:**
- [ ] Result types for recoverable errors
- [ ] Descriptive error messages
- [ ] Proper error propagation with context

**Testing:**
- [ ] Adequate test coverage (90%+ for new code)
- [ ] Edge cases tested
- [ ] Property tests for invariants
- [ ] Integration tests for new flows

**Documentation:**
- [ ] Public APIs have rustdoc
- [ ] Examples provided where helpful
- [ ] Complex logic explained with comments

**Performance:**
- [ ] No unnecessary allocations in hot paths
- [ ] Iterators used appropriately
- [ ] Arc used for shared data
- [ ] Benchmarks provided for critical changes

---

## Examples from Codebase

### Good Examples

**Sprint 5: Focused Helper Functions**
```rust
// private_poker/src/game.rs - Refactored from 235-line monolith
fn draw(&mut self, view: &GameView, frame: &mut Frame) {
    self.draw_spectators(view, frame, spectator_area);
    self.draw_waitlist(view, frame, waitlister_area);
    self.draw_table(view, frame, table_area);
}
```

**Sprint 5: Unified Macro**
```rust
// Eliminated 100+ lines of duplication
impl_user_managers!(immediate: Game<Lobby>, Game<RemovePlayers>);
impl_user_managers!(queued: Game<Deal>, Game<TakeAction>);
```

**Sprint 4: Arc Optimization**
```rust
// 8-14% performance improvement
pub struct GameView {
    pub blinds: Arc<Blinds>,
    pub board: Arc<Vec<Card>>,
    // ...
}
```

**Sprint 3: Extracted Module**
```rust
// pp_client/src/commands.rs - 30 tests, 100% coverage
pub fn parse_command(input: &str) -> Result<UserCommand, ParseError> {
    let trimmed = input.trim();
    // Clear, testable parsing logic
}
```

---

## Continuous Improvement

### Regular Reviews

- **Monthly code review**: Identify patterns, refactor opportunities
- **Quarterly refactoring sprint**: Address technical debt
- **Benchmark baseline updates**: Track performance trends

### Learning from Sprints

- **Sprint 3**: Extract and test before refactoring
- **Sprint 4**: Profile before optimizing (8-14% real improvement)
- **Sprint 5**: Small, focused functions (235 → 14 helpers)
- **Sprint 6**: Comprehensive testing catches edge cases

---

## Quick Reference

### Max Limits

| Item | Maximum | Target |
|------|---------|--------|
| Function length | 80 lines | 20-40 lines |
| Function parameters | 5 | 3 or fewer |
| Module size | 1000 lines | 500 lines |
| File size | 3500 lines | 1000 lines |

### Required Checklist

✅ Tests pass
✅ No clippy warnings
✅ Code formatted
✅ Functions < 80 lines
✅ New code tested (90%+)
✅ Public APIs documented

---

**Last Updated:** Sprint 6 Complete
**Version:** 1.0
