# Sprint 5: Code Quality & Documentation

**Duration:** 1 week
**Focus:** Improve code maintainability, reduce duplication, and enhance documentation

---

## Objectives

1. Refactor monolithic functions for better maintainability
2. Extract and modularize command parsing logic
3. Consolidate macro duplication in user management
4. Add comprehensive rustdoc to public APIs
5. Improve code organization and readability

---

## Stage 1: Refactor Large Client Functions (2 days)

### Target: `pp_client/src/app.rs`

**Issues:**
- `draw()` function: ~228 lines - too large
- Complex rendering logic mixed together
- Hard to test individual UI components

**Tasks:**
1. Extract `draw_spectators()` method
2. Extract `draw_waitlist()` method
3. Extract `draw_table()` method
4. Extract `draw_players()` method
5. Extract `draw_controls()` method
6. Simplify main `draw()` to orchestrate sub-components

**Success Criteria:**
- No function over 80 lines in `app.rs`
- Each rendering function has single responsibility
- All tests pass
- UI behavior unchanged

---

## Stage 2: Extract Command Parser Module (1 day)

### Target: `pp_client/src/app.rs` command handling

**Issues:**
- Command parsing scattered throughout input handling
- No clear separation between UI and command logic
- Hard to add new commands

**Tasks:**
1. Create `pp_client/src/commands.rs` module
2. Define `Command` enum with all commands
3. Implement `Command::parse(input: &str) -> Result<Command, ParseError>`
4. Add comprehensive tests for command parsing
5. Refactor app.rs to use command module

**Success Criteria:**
- All command parsing logic in dedicated module
- 20+ tests for command parsing
- Easy to add new commands
- Clear error messages for invalid commands

---

## Stage 3: Consolidate User Management Macros (1 day)

### Target: `private_poker/src/game.rs` macro duplication

**Issues:**
- Two macros: `impl_user_managers!` and `impl_queue_user_managers!`
- ~80% code duplication between them
- Hard to maintain and modify

**Tasks:**
1. Analyze differences between the two macros
2. Create unified macro with optional behavior flags
3. Refactor to eliminate duplication
4. Add macro tests if possible
5. Verify all game logic still works

**Success Criteria:**
- Single unified macro or better abstraction
- No behavior changes
- All tests pass
- Code reduction: -100+ lines

---

## Stage 4: Add Rustdoc to Public APIs (2 days)

### Target: All public APIs in `private_poker` crate

**Tasks:**

#### Day 1: Core Game APIs
1. Document `game.rs` public APIs:
   - `PokerState` and all state structs
   - Trait methods (GameStateManagement, etc.)
   - Key public functions
2. Document `entities.rs`:
   - All public structs (Card, Player, User, etc.)
   - Public enums (Action, PlayerState, etc.)
   - Key methods

#### Day 2: Networking and Utilities
1. Document `net/server.rs`:
   - `ServerConfig`
   - `run()` function
   - Public error types
2. Document `net/client.rs`:
   - `Client` struct and methods
3. Document `functional.rs`:
   - `eval()` - hand evaluation
   - `argmax()` - hand comparison
   - `prepare_hand()` - hand preparation

**Documentation Standards:**
```rust
/// Brief one-line description.
///
/// More detailed explanation of what this does,
/// including algorithm details if relevant.
///
/// # Arguments
///
/// * `param1` - Description of parameter
/// * `param2` - Description of parameter
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// When this function returns an error (if applicable)
///
/// # Examples
///
/// ```
/// use private_poker::eval;
/// let hand = eval(&cards);
/// ```
///
/// # Panics
///
/// When this function panics (if applicable)
pub fn function_name(param1: Type1, param2: Type2) -> Result<ReturnType, Error> {
```

**Success Criteria:**
- All public APIs have rustdoc comments
- Examples compile and work
- `cargo doc --no-deps --open` shows complete documentation
- No missing docs warnings

---

## Stage 5: Code Organization & Cleanup (1 day)

### Tasks:

1. **Module Organization:**
   - Review module structure
   - Consider splitting large modules
   - Ensure logical grouping

2. **Code Comments:**
   - Add inline comments for complex algorithms
   - Document non-obvious design decisions
   - Explain "why" not just "what"

3. **Naming Improvements:**
   - Review and improve unclear names
   - Ensure consistency across codebase
   - Follow Rust naming conventions

4. **Dead Code Removal:**
   - Remove any unused code
   - Clean up commented-out code
   - Remove debug prints

5. **Import Organization:**
   - Group imports logically
   - Remove unused imports
   - Use consistent import style

**Success Criteria:**
- Code is self-documenting where possible
- Complex logic has explanatory comments
- No clippy warnings for naming or style
- Clean, professional codebase

---

## Stage 6: Documentation & Examples (1 day)

### Tasks:

1. **Update CLAUDE.md:**
   - Add code quality guidelines
   - Document coding standards
   - Add examples of good patterns

2. **Create ARCHITECTURE.md:**
   - Document FSM state machine design
   - Explain trait-based behavior system
   - Describe networking architecture
   - Add diagrams where helpful

3. **Add Examples:**
   - Create `examples/` directory
   - Add example programs:
     - `basic_game.rs` - Simple programmatic game
     - `bot_match.rs` - Two bots playing
     - `hand_evaluation.rs` - Using eval()

4. **Update README:**
   - Add badges (tests, coverage, docs)
   - Improve getting started section
   - Add link to generated docs

**Success Criteria:**
- Comprehensive architecture documentation
- Working examples that compile
- Easy onboarding for new contributors
- Professional documentation standards

---

## Deliverables

1. **Code:**
   - All client UI functions under 80 lines
   - Dedicated command parser module with tests
   - Consolidated user management macros
   - Full rustdoc coverage for public APIs

2. **Documentation:**
   - ARCHITECTURE.md with design details
   - Updated CLAUDE.md with standards
   - 3+ working examples
   - Generated rustdoc accessible via `cargo doc`

3. **Tests:**
   - 20+ new command parser tests
   - Existing tests still passing
   - Examples compile and run

4. **Metrics:**
   - Code reduction: -200+ lines via deduplication
   - Documentation coverage: 100% of public APIs
   - Largest function: < 80 lines
   - Clippy warnings: 0

---

## Success Criteria

- ✅ All functions under 80 lines
- ✅ Command parsing is modular and tested
- ✅ No macro duplication
- ✅ 100% rustdoc coverage for public APIs
- ✅ Architecture clearly documented
- ✅ Working examples provided
- ✅ All tests passing
- ✅ No clippy warnings

---

## Timeline

| Stage | Duration | Focus |
|-------|----------|-------|
| Stage 1 | 2 days | Refactor client UI functions |
| Stage 2 | 1 day | Extract command parser |
| Stage 3 | 1 day | Consolidate macros |
| Stage 4 | 2 days | Add rustdoc |
| Stage 5 | 1 day | Code cleanup |
| Stage 6 | 1 day | Documentation & examples |
| **Total** | **8 days** | **Code quality sprint** |

---

## Notes

- Focus on maintainability over cleverness
- Document "why" not just "what"
- Keep all existing functionality working
- Run tests after each change
- Commit frequently with clear messages

---

**Sprint Status:** Ready to begin
**Dependencies:** Sprint 4 complete ✅
**Risk Level:** Low - refactoring existing code
