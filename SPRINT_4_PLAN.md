# Sprint 4: Performance & Testing - Detailed Plan

**Duration:** 1 week
**Approach:** Incremental stages with full testing and git commits after each stage

---

## Stage Overview

Each stage must:
1. ✅ Complete the implementation
2. ✅ Run full test suite (`cargo test --all`)
3. ✅ Run clippy (`cargo clippy`)
4. ✅ Build all binaries (`cargo build --all --release`)
5. ✅ Commit to git with descriptive message

---

## Stage 1: Property-Based Tests for Hand Evaluation
**Estimated Time:** 2-3 hours
**Files:** `private_poker/tests/hand_evaluation_tests.rs` (new file)

**Goals:**
- Add 20+ property-based tests using `proptest` crate
- Test hand evaluation correctness
- Test hand comparison ordering

**Tests to add:**
- Royal flush always beats everything
- Straight flush beats four of a kind
- Full house beats flush
- Pair beats high card
- Test all ranking relationships

**Success Criteria:**
- All new tests pass
- All existing tests still pass
- No clippy warnings

**Git commit:** "test: Add property-based tests for hand evaluation (Sprint 4 - Stage 1)"

---

## Stage 2: Integration Tests for Game Flow
**Estimated Time:** 2-3 hours
**Files:** `private_poker/tests/game_flow_tests.rs` (new file)

**Goals:**
- Add 10+ integration tests for full game scenarios
- Test complete game rounds (lobby → deal → showdown → distribute)
- Test edge cases (all players fold, all-in scenarios, etc.)

**Tests to add:**
- Full game round with 2 players
- Full game round with 10 players
- All players fold except one
- All players all-in
- Player joins mid-game
- Player disconnects mid-game

**Success Criteria:**
- All new tests pass
- All existing tests still pass
- No clippy warnings

**Git commit:** "test: Add integration tests for game flow (Sprint 4 - Stage 2)"

---

## Stage 3: Benchmark Framework Setup
**Estimated Time:** 1-2 hours
**Files:** `private_poker/benches/game_benchmarks.rs` (new file)

**Goals:**
- Set up criterion benchmarking
- Benchmark view generation
- Benchmark hand evaluation
- Benchmark pot distribution

**Benchmarks to add:**
- View generation with 2 players
- View generation with 10 players
- Hand evaluation (100 iterations)
- Pot distribution with side pots

**Success Criteria:**
- Benchmarks run successfully
- Baseline metrics captured
- Documentation of current performance

**Git commit:** "perf: Add benchmark framework with baseline metrics (Sprint 4 - Stage 3)"

---

## Stage 4: Arc-Based View Sharing (Part 1 - Read-Only Views)
**Estimated Time:** 2-3 hours
**Files:** `private_poker/src/game.rs`, `private_poker/src/net/messages.rs`

**Goals:**
- Wrap read-only parts of GameView in Arc
- Reduce cloning overhead for board, pot, and player list
- Maintain backward compatibility

**Changes:**
- Add Arc wrapper types
- Update view generation to use Arc
- Keep mutable parts as-is (player-specific data)

**Success Criteria:**
- All tests pass (no behavior change)
- Benchmarks show improvement
- No clippy warnings

**Git commit:** "perf: Add Arc-based view sharing for read-only data (Sprint 4 - Stage 4)"

---

## Stage 5: Optimize Hot Paths
**Estimated Time:** 2-3 hours
**Files:** Based on benchmark results

**Goals:**
- Optimize identified bottlenecks
- Focus on view generation if it's slow
- Focus on hand evaluation if it's slow

**Potential optimizations:**
- Cache expensive calculations
- Reduce unnecessary allocations
- Use more efficient data structures
- Lazy evaluation where possible

**Success Criteria:**
- All tests pass
- Benchmarks show 20%+ improvement
- No regression in functionality

**Git commit:** "perf: Optimize hot paths identified by benchmarks (Sprint 4 - Stage 5)"

---

## Stage 6: Code Refactoring (Extract Large Functions)
**Estimated Time:** 2-3 hours
**Files:** `private_poker/src/game.rs`

**Goals:**
- Extract overly large functions into smaller, testable units
- Target functions > 100 lines
- Maintain all existing behavior

**Targets:**
- `init_game_at_deal()` helper (if too large)
- Any state transition logic > 100 lines
- Complex validation logic

**Success Criteria:**
- All tests pass (no behavior change)
- Functions < 100 lines each
- Better code organization

**Git commit:** "refactor: Extract large functions for better maintainability (Sprint 4 - Stage 6)"

---

## Final Stage: Sprint 4 Summary & Documentation
**Estimated Time:** 1 hour
**Files:** `SPRINT_4_SUMMARY.md`

**Goals:**
- Document all changes
- Summarize performance improvements
- Create before/after benchmark comparison
- Update main README if needed

**Success Criteria:**
- Comprehensive documentation
- Clear metrics and improvements
- All stages committed to git

**Git commit:** "docs: Add Sprint 4 summary with performance metrics (Sprint 4 - Complete)"

---

## Testing Checklist (Run After Each Stage)

```bash
# 1. Run all tests
cargo test --all

# 2. Run clippy
cargo clippy --all-targets

# 3. Build all binaries
cargo build --all --release

# 4. Run benchmarks (stages 3+)
cargo bench

# 5. Check git status
git status

# 6. Commit changes
git add .
git commit -m "..."
```

---

## Expected Outcomes

**Performance:**
- 30-50% reduction in view generation time
- 20%+ reduction in clone overhead
- Faster hand evaluation (if optimized)

**Testing:**
- 30+ new tests added
- Property-based test coverage
- Integration test coverage
- Benchmark framework established

**Code Quality:**
- Better function organization
- More maintainable code
- Comprehensive documentation

---

## Rollback Plan

If any stage fails:
1. Run tests to identify failures
2. Review changes carefully
3. If unfixable quickly, revert the stage: `git reset --hard HEAD~1`
4. Document why it failed
5. Skip to next stage or adjust approach

---

## Success Criteria for Sprint 4

- ✅ All 6 stages completed
- ✅ All tests passing (67+ tests)
- ✅ Performance improvements measured and documented
- ✅ All changes committed to git
- ✅ No regressions in functionality
- ✅ Comprehensive documentation

**Ready to begin Stage 1!**
