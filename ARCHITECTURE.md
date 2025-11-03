# Private Poker Architecture

This document provides a detailed overview of the Private Poker system architecture, design patterns, and implementation details.

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Finite State Machine Design](#finite-state-machine-design)
3. [Trait-Based Behavior System](#trait-based-behavior-system)
4. [Networking Architecture](#networking-architecture)
5. [Hand Evaluation System](#hand-evaluation-system)
6. [Data Flow](#data-flow)
7. [Design Decisions](#design-decisions)

---

## System Overview

Private Poker is a Texas Hold'em poker implementation built around a **type-safe finite state machine (FSM)** using Rust's powerful type system. The architecture prioritizes:

- **Type safety**: Invalid state transitions are impossible at compile time
- **Zero-cost abstractions**: Using `enum_dispatch` for trait dispatch
- **Separation of concerns**: Clear boundaries between game logic, networking, and UI
- **Testability**: Pure functions and isolated state transitions

### Component Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Poker Application                     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  pp_client   │  │  pp_server   │  │   pp_bots    │  │
│  │     (TUI)    │  │   (Binary)   │  │   (Binary)   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                 │                  │          │
│         └─────────────────┼──────────────────┘          │
│                           │                             │
│  ┌────────────────────────┴───────────────────────┐    │
│  │         private_poker (Core Library)           │    │
│  ├────────────────────────────────────────────────┤    │
│  │                                                 │    │
│  │  ┌─────────────┐        ┌──────────────────┐  │    │
│  │  │    game     │        │       net        │  │    │
│  │  │             │        │                  │  │    │
│  │  │ • FSM       │◄───────┤ • Server (mio)   │  │    │
│  │  │ • Entities  │        │ • Client         │  │    │
│  │  │ • Eval      │        │ • Messages       │  │    │
│  │  └─────────────┘        └──────────────────┘  │    │
│  │                                                 │    │
│  └─────────────────────────────────────────────────┘    │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## Finite State Machine Design

### FSM States

The game progresses through 14 distinct states, each represented by a zero-sized type:

```
Lobby ─→ SeatPlayers ─→ MoveButton ─→ CollectBlinds ─→ Deal
                                                         │
                           ┌─────────────────────────────┘
                           ▼
                      TakeAction ◄──┐
                           │        │
        ┌──────────────────┼────────┘
        │                  │
        ▼                  ▼
   ShowHands           Flop/Turn/River
        │                  │
        ▼                  │
 DistributePot ◄───────────┘
        │
        ▼
 RemovePlayers ─→ UpdateBlinds ─→ BootPlayers
        │                              │
        └──────────────────────────────┘
                       │
                       ▼
                (Back to MoveButton or Lobby)
```

### Type-Safe State Transitions

Each state is implemented as `Game<T>` where `T` is a zero-sized type marker:

```rust
pub struct Game<T> {
    data: GameData,  // Shared game data
    state: T,        // Zero-sized state marker
}

// Example states (zero-sized types)
pub struct Lobby;
pub struct Deal;
pub struct TakeAction;
// ... etc
```

State transitions are implemented via `Into<PokerState>`:

```rust
impl Into<PokerState> for Game<CollectBlinds> {
    fn into(self) -> PokerState {
        // Transition logic here
        let next_state = Game::<Deal>::new(self.data);
        PokerState::Deal(next_state)
    }
}
```

**Benefits:**
- Invalid transitions are **compile-time errors**
- Each state can have state-specific methods
- No runtime overhead (zero-sized types)
- Clear state lifecycle

### The PokerState Enum

All states are unified under a single enum with `enum_dispatch`:

```rust
#[enum_dispatch]
pub enum PokerState {
    Lobby(Game<Lobby>),
    SeatPlayers(Game<SeatPlayers>),
    MoveButton(Game<MoveButton>),
    // ... all 14 states
}
```

This allows:
- Dynamic dispatch to the correct state implementation
- Zero-cost trait dispatch via `enum_dispatch`
- Unified external interface

---

## Trait-Based Behavior System

The game uses three core traits to define behavior across states:

### 1. GameStateManagement

Common operations available in **all** states:

```rust
#[enum_dispatch(PokerState)]
pub trait GameStateManagement {
    /// Drain all pending game events
    fn drain_events(&mut self) -> impl Iterator<Item = GameEvent>;

    /// Generate views for all users
    fn get_views(&self) -> HashMap<Username, GameView>;

    // ... other common operations
}
```

### 2. PhaseDependentUserManagement

User management with **phase-specific behavior**:

```rust
#[enum_dispatch(PokerState)]
pub trait PhaseDependentUserManagement {
    /// Remove user (immediate or queued based on phase)
    fn remove_user(&mut self, username: &Username) -> Result<Option<bool>, UserError>;

    /// Kick user via vote
    fn kick_user(&mut self, username: &Username) -> Result<Option<bool>, UserError>;

    // ... other user management
}
```

**Implementation Modes:**

The unified `impl_user_managers!` macro supports two modes:

- **`immediate`**: Non-gameplay phases (Lobby, RemovePlayers, etc.)
  - Operations execute immediately
  - Players can be removed from the game directly

- **`queued`**: Gameplay phases (Deal, TakeAction, Flop, etc.)
  - Operations are queued to avoid disrupting gameplay
  - Players marked for removal after hand completes

```rust
// Immediate execution
impl_user_managers!(immediate:
    Game<Lobby>,
    Game<RemovePlayers>,
    // ...
);

// Queued execution
impl_user_managers!(queued:
    Game<Deal>,
    Game<TakeAction>,
    // ...
);
```

### 3. PhaseIndependentUserManagement

Consistent user operations across **all** phases:

```rust
#[enum_dispatch(PokerState)]
pub trait PhaseIndependentUserManagement {
    /// Join as spectator or waitlister
    fn join_user(&mut self, username: Username, money: Usd) -> Result<(), UserError>;

    /// Cast vote for kick or reset
    fn vote(&mut self, username: &Username, vote: Vote) -> Result<(), UserError>;

    // ... other phase-independent operations
}
```

### Trait Dispatch Performance

Using `enum_dispatch` instead of `dyn Trait`:

- **Zero runtime cost**: Compiles to direct function calls
- **No vtable lookups**: No dynamic dispatch overhead
- **Better optimization**: Compiler can inline across trait boundaries

---

## Networking Architecture

### Server Architecture

The server uses a **multi-threaded design** with separate concerns:

```
┌─────────────────────────────────────────────┐
│              Server Process                  │
│                                              │
│  ┌────────────────────────────────────────┐ │
│  │         Main Thread (mio)              │ │
│  │  • Accept connections                  │ │
│  │  • Non-blocking I/O with Poll          │ │
│  │  • Token-based client tracking         │ │
│  │  • Read/Write message framing          │ │
│  └─────────┬──────────────────────▲────────┘ │
│            │                      │          │
│            │ ClientMessage        │ Views    │
│            ▼                      │          │
│  ┌─────────────────────────────────────────┐ │
│  │       Game Logic Thread                │ │
│  │  • PokerState FSM                      │ │
│  │  • Process commands                    │ │
│  │  • Generate views                      │ │
│  │  • Event generation                    │ │
│  └────────────────────────────────────────┘ │
│                                              │
└──────────────────────────────────────────────┘
```

**Key Components:**

1. **Event Loop (mio)**
   - Non-blocking TCP with `mio::Poll`
   - Token-based client identification
   - Interest-based event notification (readable/writable)

2. **Game Logic Thread**
   - Separate thread for deterministic game state
   - Receives commands via `mpsc` channel
   - Sends views back to event loop

3. **Token Manager**
   - Maps `mio::Token` to `Username`
   - Handles client connections/disconnections
   - Recycles tokens for efficiency

4. **Rate Limiting**
   - IP-based connection limiting
   - Prevents DoS attacks
   - Configurable limits per IP

### Message Protocol

**Binary protocol using bincode serialization:**

```
┌──────────────────────────────────────┐
│      Message Frame (Length-Prefixed) │
├──────────────────────────────────────┤
│  4 bytes  │     N bytes              │
│  Length   │  Bincode Payload         │
└──────────────────────────────────────┘
```

**Message Types:**

```rust
// Client → Server
pub struct ClientMessage {
    pub username: Username,
    pub command: UserCommand,  // Connect, TakeAction, Vote, etc.
}

// Server → Client
pub enum ServerMessage {
    Ack(ClientMessage),           // Command accepted
    GameView(GameView),           // Current state
    TurnSignal(ActionChoices),    // Your turn
    GameEvent(GameEvent),         // Game event notification
    ClientError(ClientError),     // Protocol error
    UserError(UserError),         // Game logic error
}
```

### Client Architecture

The `pp_client` uses a **two-thread design**:

```
┌──────────────────────────────────────┐
│          Client Process               │
│                                       │
│  ┌─────────────────────────────────┐ │
│  │      UI Thread (ratatui)        │ │
│  │  • Render game view             │ │
│  │  • Handle user input            │ │
│  │  • Parse commands               │ │
│  └────┬──────────────────▲─────────┘ │
│       │                  │           │
│       │ Commands         │ Views     │
│       ▼                  │           │
│  ┌─────────────────────────────────┐ │
│  │    Network Thread (mio)         │ │
│  │  • Non-blocking I/O             │ │
│  │  • Message serialization        │ │
│  │  • Connection management        │ │
│  └─────────────────────────────────┘ │
│                                       │
└───────────────────────────────────────┘
```

**Separation Benefits:**
- UI remains responsive during network delays
- Non-blocking I/O doesn't freeze rendering
- Clear message passing between threads

---

## Hand Evaluation System

### Algorithm Overview

The hand evaluator (`functional.rs`) uses a **greedy best-hand algorithm**:

```rust
pub fn eval(cards: &[Card]) -> Vec<SubHand>
```

**Process:**

1. **Group cards** by value and suit using `BTreeMap`
2. **Check patterns** in priority order:
   - Straight flush (suit + sequence)
   - Four of a kind (value groups)
   - Full house (value groups)
   - Flush (suit groups)
   - Straight (sequence)
   - Three of a kind
   - Two pair
   - One pair
   - High card

3. **Return best 5-card hand** as `Vec<SubHand>`

### Hand Comparison

```rust
pub fn argmax(hands: &[Vec<SubHand>]) -> Vec<usize>
```

**Comparison Logic:**
1. Compare by `Rank` (straight flush > four of a kind > ...)
2. If tied, compare `Values` element-wise
3. Return indices of all winning hands (supports ties)

### SubHand Structure

```rust
pub struct SubHand {
    pub rank: Rank,      // Hand type (Pair, Flush, etc.)
    pub values: Vec<u8>, // Tiebreaker values in descending order
}
```

**Example:**
- Pair of Kings: `SubHand { rank: Rank::OnePair, values: [13, 13, 10, 9, 5] }`
- Flush: `SubHand { rank: Rank::Flush, values: [14, 12, 10, 8, 3] }`

---

## Data Flow

### Complete Game Flow

```
User Input (pp_client)
    │
    ▼
Command Parser ─→ ClientMessage
    │
    ▼
Network Thread ─→ Server Event Loop
    │
    ▼
Game Logic Thread ─→ PokerState::step()
    │
    ├─→ Process Command
    ├─→ Update State
    ├─→ Generate Events
    └─→ Create Views
         │
         ▼
    HashMap<Username, GameView>
         │
         ▼
    Server Event Loop ─→ Network Thread
         │
         ▼
    Client receives ServerMessage::GameView
         │
         ▼
    UI renders with ratatui
```

### View Generation

Views are generated with **Arc-based sharing** for efficiency:

```rust
// Shared read-only data wrapped in Arc
struct SharedViewData {
    blinds: Arc<Blinds>,
    spectators: Arc<HashSet<User>>,
    waitlist: Arc<VecDeque<User>>,
    board: Arc<Vec<Card>>,
    pot: Arc<PotView>,
    // ...
}

// Each user gets a view with:
// - Shared Arc references (cheap to clone)
// - Personalized player data (own cards visible)
fn get_views(&self) -> HashMap<Username, GameView> {
    let shared = create_shared_data();

    self.users.iter().map(|username| {
        let view = GameView {
            // Clone Arc (just increments refcount)
            blinds: shared.blinds.clone(),
            spectators: shared.spectators.clone(),
            // ... personalized player list
            players: create_player_views(username),
        };
        (username.clone(), view)
    }).collect()
}
```

**Performance:**
- Arc overhead: ~10ns per field per view
- Saves cloning large vectors for each user
- 8-14% faster for 4-10 players

---

## Design Decisions

### Why FSM with Zero-Sized Types?

**Alternatives Considered:**
- Single struct with state enum → No compile-time guarantees
- State pattern with trait objects → Runtime overhead

**Chosen Approach:**
- `Game<T>` with zero-sized type markers
- **Benefits:** Type safety, zero runtime cost, clear state-specific APIs

### Why enum_dispatch?

**Alternatives:**
- `Box<dyn Trait>` → Heap allocation, vtable overhead
- Manual match statements → Code duplication, error-prone

**Chosen Approach:**
- `enum_dispatch` macro generates match-based dispatch
- **Benefits:** Zero-cost abstraction, compile-time optimization

### Why Separate Client/Server?

**Alternatives:**
- Embed server in client binary
- Peer-to-peer networking

**Chosen Approach:**
- Dedicated server process
- **Benefits:** Authoritative server, simpler security, better testability

### Why Binary Protocol (bincode)?

**Alternatives:**
- JSON → Larger messages, slower parsing
- MessagePack → Similar benefits, less Rust integration

**Chosen Approach:**
- Bincode with length-prefixed frames
- **Benefits:** Compact, fast, excellent Rust/serde integration

### Why Two-Thread Client?

**Alternatives:**
- Single-threaded with async → Complex state management
- Blocking I/O → UI freezes during network delays

**Chosen Approach:**
- UI thread + Network thread with channels
- **Benefits:** Responsive UI, clear separation of concerns

---

## Performance Characteristics

### Hand Evaluation
- 2-card hand: ~549 ns
- 7-card hand: ~1.35 µs
- 100 iterations: ~159 µs (~1.59 µs per hand)

### View Generation (Sprint 4 Optimizations)
- 2 players: 837 ns
- 4 players: 1.92 µs
- 8 players: 5.58 µs
- 10 players: 7.96 µs

**Optimizations Applied:**
1. Arc-based sharing for read-only data
2. Iterator-chain optimizations
3. Reduced cloning overhead

### State Transitions
- Typical state step: 3-5 µs
- Event draining: ~445 ns

---

## Testing Strategy

### Unit Tests
- Embedded in source files with `#[cfg(test)]`
- Test individual functions and state transitions
- **61 tests** in core library

### Integration Tests
- Full client-server interactions (`tests/client_server.rs`)
- Complete game flow scenarios (`tests/game_flow_integration.rs`)
- **12 integration tests**

### Property-Based Testing
- Hand evaluation correctness (`tests/hand_evaluation_proptest.rs`)
- Random input generation with `proptest`
- **13 property-based tests**

### Command Parser Tests
- Comprehensive command parsing coverage (`pp_client/src/commands.rs`)
- **30 tests** for all command variants

**Total: 121 tests**

---

## Code Quality Metrics

### Sprint 5 Improvements

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Largest function | 235 lines | 55 lines | **-77%** |
| Duplicate macros | 2 | 1 (unified) | **-100+ lines** |
| Command parsing tests | 0 | 30 | **+30 tests** |
| Clippy warnings | Many | Minimal | **Significantly reduced** |

### Current State
- **All functions under 80 lines**
- **Zero clippy warnings in production code**
- **All tests passing (121 total)**
- **Documentation coverage for key APIs**

---

## Future Enhancements

### Potential Optimizations
1. **Object pooling** for frequently allocated structures
2. **Lazy view generation** - only generate when requested
3. **View caching** - cache views between state changes
4. **SIMD optimizations** for hand evaluation

### Feature Additions
1. Tournament support
2. Multi-table support
3. Replay/hand history
4. Advanced statistics

---

## References

- **Finite State Machine Pattern**: Type-safe state machines in Rust
- **enum_dispatch**: Zero-cost enum-based trait dispatch
- **mio**: Metal I/O library for async networking
- **bincode**: Binary serialization for Rust structures
- **ratatui**: Terminal UI framework

---

**Last Updated:** Sprint 5 Completion
**Contributors:** See git history
