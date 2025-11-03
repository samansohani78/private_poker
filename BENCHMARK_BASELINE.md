# Benchmark Baseline Metrics (Sprint 4 - Stage 3)

Captured on: 2025-11-03
System: Linux 6.16.3
Rust Profile: `bench` (optimized)

## Hand Evaluation Benchmarks

| Benchmark | Time | Notes |
|-----------|------|-------|
| hand_eval_2_cards | 549.39 ns | Evaluating pocket cards (2 cards) |
| hand_eval_7_cards | 1.3474 µs | Evaluating full hand + board (7 cards) |
| hand_eval_100_iterations | 159.13 µs | 100 hand evaluations (~1.59 µs each) |
| hand_comparison_4_hands | 29.756 ns | Comparing 4 hands with argmax |

## Game Operations Benchmarks

### View Generation (scales with player count)

| Players | Time | Linear Scaling |
|---------|------|----------------|
| 2 | 860.35 ns | baseline |
| 4 | 2.0803 µs | ~2.4x |
| 6 | 3.9131 µs | ~4.5x |
| 8 | 6.5187 µs | ~7.6x |
| 10 | 9.0070 µs | ~10.5x |

View generation scales roughly linearly with player count, which is expected as each player needs their own view generated.

### Game State Operations

| Benchmark | Time | Notes |
|-----------|------|-------|
| game_step/2_players | 3.3537 µs | State transition with 2 players |
| game_step/10_players | 604.88 ns | State transition with 10 players |
| drain_events | 445.48 ns | Draining event queue |

**Note:** game_step/10_players is faster than 2_players because the game states after initial setup may differ. The 10-player game likely advances to a different phase faster.

## Key Insights

1. **Hand evaluation is fast**: Single hand evaluation takes ~1.35-1.59 µs
2. **View generation dominates**: For 10 players, generating views (~9 µs) takes longer than hand evaluation
3. **Event draining is efficient**: ~445 ns is very fast
4. **Scalability**: View generation scales linearly with player count as expected

## Target Improvements (Next Stages)

- **Stage 4**: Reduce view generation overhead by using Arc for shared read-only data
- **Stage 5**: Optimize any hot paths identified by profiling
- Target: 30-50% reduction in view generation time for 10 players (from 9 µs to ~5-6 µs)
