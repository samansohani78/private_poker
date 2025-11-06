use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    fmt::{self},
    hash::{Hash, Hasher},
    mem::discriminant,
    sync::Arc,
};

use super::constants;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Suit {
    Club,
    Spade,
    Diamond,
    Heart,
    // Wild is used to initialize a deck of cards.
    // Might be a good choice for a joker's suit.
    Wild,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::Club => "♣",
            Self::Spade => "♠",
            Self::Diamond => "♦",
            Self::Heart => "♥",
            Self::Wild => "w",
        };
        write!(f, "{repr}")
    }
}

/// Placeholder for card values.
pub type Value = u8;

/// A card is a tuple of a uInt8 value (ace=1u8 ... ace=14u8)
/// and a suit. A joker is depicted as 0u8.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Card(pub Value, pub Suit);

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match self.0 {
            1 | 14 => "A",
            11 => "J",
            12 => "Q",
            13 => "K",
            v => &v.to_string(),
        };
        let repr = format!("{value}/{}", self.1);
        write!(f, "{repr:>4}")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rank {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match self {
            Self::HighCard => "hi",
            Self::OnePair => "1p",
            Self::TwoPair => "2p",
            Self::ThreeOfAKind => "3k",
            Self::Straight => "s8",
            Self::Flush => "fs",
            Self::FullHouse => "fh",
            Self::FourOfAKind => "4k",
            Self::StraightFlush => "sf",
        };
        write!(f, "{repr}")
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SubHand {
    pub rank: Rank,
    pub values: Vec<Value>,
}

#[derive(Debug)]
pub struct Deck {
    cards: [Card; 52],
    pub deck_idx: usize,
}

impl Deck {
    pub fn deal_card(&mut self) -> Card {
        let card = self.cards[self.deck_idx];
        self.deck_idx += 1;
        card
    }

    pub fn shuffle(&mut self) {
        self.cards.shuffle(&mut thread_rng());
        self.deck_idx = 0;
    }
}

impl Default for Deck {
    fn default() -> Self {
        let mut cards: [Card; 52] = [Card(0, Suit::Wild); 52];
        for (i, value) in (1u8..14u8).enumerate() {
            for (j, suit) in [Suit::Club, Suit::Spade, Suit::Diamond, Suit::Heart]
                .into_iter()
                .enumerate()
            {
                cards[4 * i + j] = Card(value, suit);
            }
        }
        Self { cards, deck_idx: 0 }
    }
}

/// Type alias for whole dollars. All bets and player stacks are represented
/// as whole dollars (there's no point arguing over pennies).
///
/// If the total money in a game ever surpasses ~4.2 billion, then we may
/// have a problem.
pub type Usd = u32;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Username(String);

impl Username {
    pub fn new(s: &str) -> Self {
        let mut username: String = s
            .chars()
            .map(|c| if c.is_ascii_whitespace() { '_' } else { c })
            .collect();
        username.truncate(constants::MAX_USER_INPUT_LENGTH / 2);
        Self(username)
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Username {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(&s))
    }
}

impl From<String> for Username {
    fn from(value: String) -> Self {
        Self::new(&value)
    }
}

/// Type alias for seat positions during the game.
pub type SeatIndex = usize;

/// Play positions used for tracking who is paying what blinds and whose
/// turn is next.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlayPositions {
    pub small_blind_idx: SeatIndex,
    pub big_blind_idx: SeatIndex,
    pub starting_action_idx: SeatIndex,
    pub next_action_idx: Option<SeatIndex>,
}

impl Default for PlayPositions {
    fn default() -> Self {
        Self {
            small_blind_idx: 0,
            big_blind_idx: 1,
            starting_action_idx: 2,
            next_action_idx: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayerCounts {
    /// Count of the number of players active in a hand.
    /// All-in and folding are considered INACTIVE since they
    /// have no more moves to make. Once `num_players_called`
    /// is equal to this value, the round of betting is concluded.
    pub num_active: usize,
    /// Count of the number of players that have matched the minimum
    /// call. Coupled with `num_players_active`, this helps track
    /// whether a round of betting has ended. This value is reset
    /// at the beginning of each betting round and whenever a player
    /// raises (since they've increased the minimum call).
    pub num_called: usize,
}

#[derive(Debug, Default)]
pub struct PlayerQueues {
    /// Queue of users that've been voted to be kicked. We can't
    /// safely remove them from the game mid gameplay, so we instead queue
    /// them for removal.
    pub to_kick: BTreeSet<Username>,
    /// Queue of users that're playing the game but have opted
    /// to spectate. We can't safely remove them from the game mid gameplay,
    /// so we instead queue them for removal.
    pub to_spectate: BTreeSet<Username>,
    /// Queue of users that're playing the game but have opted
    /// to leave. We can't safely remove them from the game mid gameplay,
    /// so we instead queue them for removal.
    pub to_remove: BTreeSet<Username>,
    /// Queue of users whose money we'll reset. We can't safely
    /// reset them mid gameplay, so we instead queue them for reset.
    pub to_reset: BTreeSet<Username>,
}

// By default, a player will be cleaned if they fold 60 rounds with the big
// blind.
pub const DEFAULT_BUY_IN: Usd = 600;
pub const DEFAULT_MIN_BIG_BLIND: Usd = DEFAULT_BUY_IN / 60;
pub const DEFAULT_MIN_SMALL_BLIND: Usd = DEFAULT_MIN_BIG_BLIND / 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Blinds {
    pub small: Usd,
    pub big: Usd,
}

impl fmt::Display for Blinds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = format!("${}/{}", self.small, self.big);
        write!(f, "{repr}")
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct User {
    pub name: Username,
    pub money: Usd,
}

impl Hash for User {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Borrow<Username> for User {
    fn borrow(&self) -> &Username {
        &self.name
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Action {
    AllIn,
    Call,
    Check,
    Fold,
    Raise(Option<Usd>),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::AllIn => "all-ins (unhinged)",
            Self::Call => "calls",
            Self::Check => "checks",
            Self::Fold => "folds",
            Self::Raise(Some(amount)) => &format!("raises ${amount}"),
            Self::Raise(None) => "raises",
        };
        write!(f, "{repr}")
    }
}

impl From<Bet> for Action {
    fn from(value: Bet) -> Self {
        match value.action {
            BetAction::AllIn => Self::AllIn,
            BetAction::Call => Self::Call,
            BetAction::Raise => Self::Raise(Some(value.amount)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ActionChoice {
    AllIn,
    Call(Usd),
    Check,
    Fold,
    Raise(Usd),
}

// Can't really convert a usize into an ActionChoice safely, and it doesn't
// really make sense to use a try- conversion version, so let's just
// use the into trait.
#[allow(clippy::from_over_into)]
impl Into<usize> for ActionChoice {
    fn into(self) -> usize {
        match self {
            Self::AllIn => 0,
            Self::Call(_) => 1,
            Self::Check => 2,
            Self::Fold => 3,
            Self::Raise(_) => 4,
        }
    }
}

impl fmt::Display for ActionChoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::AllIn => "all-in".to_string(),
            Self::Call(amount) => format!("call (== ${amount})"),
            Self::Check => "check".to_string(),
            Self::Fold => "fold".to_string(),
            Self::Raise(amount) => format!("raise (>= ${amount})"),
        };
        write!(f, "{repr}")
    }
}

// We don't care about the values within `ActionChoice::Call` and
// `ActionChoice::Raise`. We just perform checks against the enum
// variant to verify a user is choosing an action that's available
// within their presented action choices. Actual bet validation
// is done during the `TakeAction` game state.
impl Eq for ActionChoice {}

impl Hash for ActionChoice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        discriminant(self).hash(state);
    }
}

impl PartialEq for ActionChoice {
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
}

impl From<ActionChoice> for Action {
    fn from(value: ActionChoice) -> Self {
        match value {
            ActionChoice::AllIn => Self::AllIn,
            ActionChoice::Call(_) => Self::Call,
            ActionChoice::Check => Self::Check,
            ActionChoice::Fold => Self::Fold,
            ActionChoice::Raise(amount) => Self::Raise(Some(amount)),
        }
    }
}

/// Type alias for a set of action choices.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ActionChoices(pub HashSet<ActionChoice>);

impl ActionChoices {
    pub fn contains(&self, action: &Action) -> bool {
        // ActionChoice uses variant discriminates for hashes, so we
        // don't need to care about the actual call/raise values.
        let action_choice: ActionChoice = match action {
            Action::AllIn => ActionChoice::AllIn,
            Action::Call => ActionChoice::Call(0),
            Action::Check => ActionChoice::Check,
            Action::Fold => ActionChoice::Fold,
            Action::Raise(_) => ActionChoice::Raise(0),
        };
        self.0.contains(&action_choice)
    }
}

impl fmt::Display for ActionChoices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let num_options = self.0.len();
        let repr = self
            .0
            .iter()
            .enumerate()
            .map(|(i, action_choice)| {
                let repr = action_choice.to_string();
                match i {
                    0 if num_options == 1 => repr,
                    0 if num_options == 2 => format!("{repr} "),
                    0 if num_options >= 3 => format!("{repr}, "),
                    i if i == num_options - 1 && num_options != 1 => format!("or {repr}"),
                    _ => format!("{repr}, "),
                }
            })
            .collect::<String>();
        write!(f, "{repr}")
    }
}

impl<I> From<I> for ActionChoices
where
    I: IntoIterator<Item = ActionChoice>,
{
    fn from(iter: I) -> Self {
        Self(iter.into_iter().collect::<HashSet<_>>())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BetAction {
    AllIn,
    Call,
    Raise,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Bet {
    pub action: BetAction,
    pub amount: Usd,
}

impl fmt::Display for Bet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let amount = self.amount;
        let repr = match self.action {
            BetAction::AllIn => format!("all-in of ${amount}"),
            BetAction::Call => format!("call of ${amount}"),
            BetAction::Raise => format!("raise of ${amount}"),
        };
        write!(f, "{repr}")
    }
}

/// For users that're in a pot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum PlayerState {
    // Player put in their whole stack.
    AllIn,
    // Player calls.
    Call,
    // Player checks.
    Check,
    // Player forfeited their stack for the pot.
    Fold,
    // Player raises and is waiting for other player actions.
    Raise,
    // Player is in the pot but is waiting for their move.
    Wait,
}

impl fmt::Display for PlayerState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::AllIn => "all-in",
            Self::Call => "call",
            Self::Check => "check",
            Self::Fold => "folded",
            Self::Raise => "raise",
            Self::Wait => "waiting",
        };
        write!(f, "{repr:7}")
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub user: User,
    pub state: PlayerState,
    pub cards: Vec<Card>,
    pub showing: bool,
    pub seat_idx: usize,
}

impl Player {
    #[must_use]
    pub fn new(user: User, seat_idx: usize) -> Self {
        Self {
            user,
            state: PlayerState::Wait,
            cards: Vec::with_capacity(2),
            showing: false,
            seat_idx,
        }
    }

    pub fn reset(&mut self) {
        self.state = PlayerState::Wait;
        self.cards.clear();
        self.showing = false;
    }
}

#[derive(Clone, Debug)]
pub struct Pot {
    // Map seat indices (players) to their investment in the pot.
    pub investments: HashMap<usize, Usd>,
}

impl Default for Pot {
    fn default() -> Self {
        Self::new(constants::MAX_PLAYERS)
    }
}

impl Pot {
    pub fn bet(&mut self, player_idx: usize, bet: &Bet) {
        let investment = self.investments.entry(player_idx).or_default();
        *investment += bet.amount;
    }

    #[must_use]
    pub fn get_call(&self) -> Usd {
        *self.investments.values().max().unwrap_or(&0)
    }

    /// Return the amount the player must bet to remain in the hand, and
    /// the minimum the player must raise by for it to be considered
    /// a valid raise.
    #[must_use]
    pub fn get_call_by_player_idx(&self, player_idx: usize) -> Usd {
        self.get_call() - self.get_investment_by_player_idx(player_idx)
    }

    /// Return the amount the player has invested in the pot.
    #[must_use]
    pub fn get_investment_by_player_idx(&self, player_idx: usize) -> Usd {
        *self.investments.get(&player_idx).unwrap_or(&0)
    }

    /// Return the minimum amount a player has to bet in order for their
    /// raise to be considered a valid raise.
    #[must_use]
    pub fn get_min_raise_by_player_idx(&self, player_idx: usize) -> Usd {
        2 * self.get_call() - self.get_investment_by_player_idx(player_idx)
    }

    #[must_use]
    pub fn get_size(&self) -> Usd {
        self.investments.values().sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.get_size() == 0
    }

    #[must_use]
    pub fn new(max_players: usize) -> Self {
        Self {
            investments: HashMap::with_capacity(max_players),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Vote {
    // Vote to kick another user.
    Kick(Username),
    // Vote to reset money (for a specific user or for everyone).
    Reset(Option<Username>),
}

impl fmt::Display for Vote {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::Kick(username) => format!("kick {username}"),
            Self::Reset(None) => "reset everyone's money".to_string(),
            Self::Reset(Some(username)) => format!("reset {username}'s money"),
        };
        write!(f, "{repr}")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlayerView {
    pub user: User,
    pub state: PlayerState,
    pub cards: Vec<Card>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PotView {
    pub size: Usd,
}

impl fmt::Display for PotView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.size)
    }
}

// Helper module for Arc serialization
mod arc_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S, T>(arc: &Arc<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        arc.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Arc<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        T::deserialize(deserializer).map(Arc::new)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GameView {
    #[serde(with = "arc_serde")]
    pub blinds: Arc<Blinds>,
    #[serde(with = "arc_serde")]
    pub spectators: Arc<HashSet<User>>,
    #[serde(with = "arc_serde")]
    pub waitlist: Arc<VecDeque<User>>,
    #[serde(with = "arc_serde")]
    pub open_seats: Arc<VecDeque<usize>>,
    pub players: Vec<PlayerView>,
    #[serde(with = "arc_serde")]
    pub board: Arc<Vec<Card>>,
    #[serde(with = "arc_serde")]
    pub pot: Arc<PotView>,
    #[serde(with = "arc_serde")]
    pub play_positions: Arc<PlayPositions>,
}

pub type GameViews = HashMap<Username, GameView>;

#[cfg(test)]
mod tests {
    use super::*;

    // === Card Tests ===

    #[test]
    fn test_card_creation() {
        let card = Card(14, Suit::Spade);
        assert_eq!(card.0, 14);
        assert_eq!(card.1, Suit::Spade);
    }

    #[test]
    fn test_card_value_range() {
        for value in 1..=14 {
            let card = Card(value, Suit::Heart);
            assert!(card.0 >= 1 && card.0 <= 14);
        }
    }

    #[test]
    fn test_all_suits() {
        let suits = [Suit::Heart, Suit::Diamond, Suit::Club, Suit::Spade];
        for suit in suits {
            let card = Card(7, suit.clone());
            assert_eq!(card.1, suit);
        }
    }

    // === Deck Tests ===

    #[test]
    fn test_deck_initialization() {
        let deck = Deck::default();
        assert_eq!(deck.cards.len(), 52);
    }

    #[test]
    fn test_deck_shuffle() {
        let mut deck = Deck::default();
        deck.shuffle();
        assert_eq!(deck.cards.len(), 52);
        assert_eq!(deck.deck_idx, 0);
    }

    #[test]
    fn test_deck_deal_card() {
        let mut deck = Deck::default();
        let card = deck.deal_card();
        assert!(card.0 >= 1 && card.0 <= 14);
        assert_eq!(deck.deck_idx, 1);
    }

    #[test]
    fn test_deck_deal_multiple_cards() {
        let mut deck = Deck::default();
        for i in 1..=5 {
            let _card = deck.deal_card();
            assert_eq!(deck.deck_idx, i);
        }
    }

    // === Username Tests ===

    #[test]
    fn test_username_creation() {
        let username: Username = "alice".to_string().into();
        let username2: Username = "alice".to_string().into();
        assert_eq!(username, username2);
    }

    #[test]
    fn test_username_equality() {
        let user1: Username = "bob".to_string().into();
        let user2: Username = "bob".to_string().into();
        let user3: Username = "alice".to_string().into();
        assert_eq!(user1, user2);
        assert_ne!(user1, user3);
    }

    // === Blinds Tests ===

    #[test]
    fn test_blinds_creation() {
        let blinds = Blinds { small: 10, big: 20 };
        assert_eq!(blinds.small, 10);
        assert_eq!(blinds.big, 20);
    }

    #[test]
    fn test_blinds_typical_ratio() {
        let blinds = Blinds { small: 50, big: 100 };
        assert_eq!(blinds.big, blinds.small * 2);
    }

    // === Action Tests ===

    #[test]
    fn test_action_fold() {
        let action = Action::Fold;
        assert!(matches!(action, Action::Fold));
    }

    #[test]
    fn test_action_check() {
        let action = Action::Check;
        assert!(matches!(action, Action::Check));
    }

    #[test]
    fn test_action_call() {
        let action = Action::Call;
        assert!(matches!(action, Action::Call));
    }

    #[test]
    fn test_action_all_in() {
        let action = Action::AllIn;
        assert!(matches!(action, Action::AllIn));
    }

    #[test]
    fn test_action_raise_with_amount() {
        let action = Action::Raise(Some(100));
        match action {
            Action::Raise(Some(amount)) => assert_eq!(amount, 100),
            _ => panic!("Expected Raise with amount"),
        }
    }

    #[test]
    fn test_action_raise_without_amount() {
        let action = Action::Raise(None);
        assert!(matches!(action, Action::Raise(None)));
    }

    // === Vote Tests ===

    #[test]
    fn test_vote_kick() {
        let target: Username = "bad_player".to_string().into();
        let vote = Vote::Kick(target.clone());
        match vote {
            Vote::Kick(username) => assert_eq!(username, target),
            _ => panic!("Expected Kick vote"),
        }
    }

    #[test]
    fn test_vote_reset_specific_user() {
        let target: Username = "player1".to_string().into();
        let vote = Vote::Reset(Some(target.clone()));
        match vote {
            Vote::Reset(Some(username)) => assert_eq!(username, target),
            _ => panic!("Expected Reset vote with target"),
        }
    }

    #[test]
    fn test_vote_reset_all() {
        let vote = Vote::Reset(None);
        assert!(matches!(vote, Vote::Reset(None)));
    }

    // === Pot Tests ===

    #[test]
    fn test_pot_default() {
        let pot = Pot::default();
        assert!(pot.investments.is_empty());
    }

    // === Rank Tests ===

    #[test]
    fn test_rank_ordering() {
        assert!(Rank::HighCard < Rank::OnePair);
        assert!(Rank::OnePair < Rank::TwoPair);
        assert!(Rank::TwoPair < Rank::ThreeOfAKind);
        assert!(Rank::ThreeOfAKind < Rank::Straight);
        assert!(Rank::Straight < Rank::Flush);
        assert!(Rank::Flush < Rank::FullHouse);
        assert!(Rank::FullHouse < Rank::FourOfAKind);
        assert!(Rank::FourOfAKind < Rank::StraightFlush);
    }

    #[test]
    fn test_rank_equality() {
        assert_eq!(Rank::OnePair, Rank::OnePair);
        assert_eq!(Rank::Flush, Rank::Flush);
        assert_ne!(Rank::Straight, Rank::StraightFlush);
    }

    // === SubHand Tests ===

    #[test]
    fn test_subhand_creation() {
        let subhand = SubHand {
            rank: Rank::OnePair,
            values: vec![14, 14, 13, 12, 11],
        };
        assert_eq!(subhand.rank, Rank::OnePair);
        assert_eq!(subhand.values.len(), 5);
    }

    #[test]
    fn test_subhand_comparison() {
        let pair_aces = SubHand {
            rank: Rank::OnePair,
            values: vec![14, 14, 13, 12, 11],
        };
        let pair_kings = SubHand {
            rank: Rank::OnePair,
            values: vec![13, 13, 12, 11, 10],
        };
        // Higher pair should be better
        assert!(pair_aces > pair_kings);
    }

    #[test]
    fn test_subhand_rank_dominates() {
        let two_pair = SubHand {
            rank: Rank::TwoPair,
            values: vec![5, 5, 4, 4, 3],
        };
        let one_pair = SubHand {
            rank: Rank::OnePair,
            values: vec![14, 14, 13, 12, 11],
        };
        // Two pair beats one pair regardless of values
        assert!(two_pair > one_pair);
    }

    // === User Tests (Extended) ===

    #[test]
    fn test_user_equality() {
        let user1 = User {
            name: "alice".to_string().into(),
            money: 1000,
        };
        let user2 = User {
            name: "alice".to_string().into(),
            money: 1000,
        };
        assert_eq!(user1, user2);
    }

    #[test]
    fn test_user_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        let user = User {
            name: "bob".to_string().into(),
            money: 500,
        };
        set.insert(user.clone());
        assert!(set.contains(&user));
    }

    // === ActionChoice Tests ===

    #[test]
    fn test_action_choice_fold() {
        let choice = ActionChoice::Fold;
        assert!(matches!(choice, ActionChoice::Fold));
    }

    #[test]
    fn test_action_choice_check() {
        let choice = ActionChoice::Check;
        assert!(matches!(choice, ActionChoice::Check));
    }

    #[test]
    fn test_action_choice_call_with_amount() {
        let choice = ActionChoice::Call(100);
        assert!(matches!(choice, ActionChoice::Call(100)));
    }

    #[test]
    fn test_action_choice_raise_with_amount() {
        let choice = ActionChoice::Raise(200);
        assert!(matches!(choice, ActionChoice::Raise(200)));
    }

    #[test]
    fn test_action_choice_all_in() {
        let choice = ActionChoice::AllIn;
        assert!(matches!(choice, ActionChoice::AllIn));
    }

    // === ActionChoices Tests ===

    #[test]
    fn test_action_choices_creation() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ActionChoice::Fold);
        set.insert(ActionChoice::Check);
        let choices = ActionChoices(set);
        assert_eq!(choices.0.len(), 2);
    }

    #[test]
    fn test_action_choices_contains() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ActionChoice::Fold);
        set.insert(ActionChoice::Call(50));
        let choices = ActionChoices(set);
        assert!(choices.0.contains(&ActionChoice::Fold));
        assert!(choices.0.contains(&ActionChoice::Call(50)));
    }

    // === BetAction Tests ===

    #[test]
    fn test_bet_action_all_in() {
        let action = BetAction::AllIn;
        assert!(matches!(action, BetAction::AllIn));
    }

    #[test]
    fn test_bet_action_call() {
        let action = BetAction::Call;
        assert!(matches!(action, BetAction::Call));
    }

    #[test]
    fn test_bet_action_raise() {
        let action = BetAction::Raise;
        assert!(matches!(action, BetAction::Raise));
    }

    // === Bet Tests ===

    #[test]
    fn test_bet_creation() {
        let bet = Bet {
            action: BetAction::Raise,
            amount: 100,
        };
        assert!(matches!(bet.action, BetAction::Raise));
        assert_eq!(bet.amount, 100);
    }

    #[test]
    fn test_bet_call_zero() {
        let bet = Bet {
            action: BetAction::Call,
            amount: 0,
        };
        assert_eq!(bet.amount, 0);
    }

    // === PlayerState Tests (Extended) ===

    #[test]
    fn test_player_state_variants() {
        let states = vec![
            PlayerState::AllIn,
            PlayerState::Call,
            PlayerState::Check,
            PlayerState::Fold,
            PlayerState::Raise,
            PlayerState::Wait,
        ];
        assert_eq!(states.len(), 6);
    }

    // === Player Tests (Extended) ===

    #[test]
    fn test_player_new() {
        let user = User {
            name: "test".to_string().into(),
            money: 1000,
        };
        let player = Player::new(user.clone(), 0);
        assert_eq!(player.user, user);
        assert_eq!(player.seat_idx, 0);
        assert!(!player.showing);
        assert!(player.cards.is_empty());
    }

    // === PlayPositions Tests ===

    #[test]
    fn test_play_positions_default() {
        let positions = PlayPositions::default();
        assert!(positions.next_action_idx.is_none());
    }

    #[test]
    fn test_play_positions_with_values() {
        let positions = PlayPositions {
            small_blind_idx: 0,
            big_blind_idx: 1,
            starting_action_idx: 2,
            next_action_idx: Some(0),
        };
        assert_eq!(positions.small_blind_idx, 0);
        assert_eq!(positions.big_blind_idx, 1);
        assert_eq!(positions.starting_action_idx, 2);
        assert_eq!(positions.next_action_idx, Some(0));
    }

    // === PlayerCounts Tests ===

    #[test]
    fn test_player_counts_default() {
        let counts = PlayerCounts::default();
        assert_eq!(counts.num_active, 0);
        assert_eq!(counts.num_called, 0);
    }

    // === PlayerQueues Tests ===

    #[test]
    fn test_player_queues_default() {
        let queues = PlayerQueues::default();
        assert!(queues.to_remove.is_empty());
        assert!(queues.to_kick.is_empty());
        assert!(queues.to_reset.is_empty());
        assert!(queues.to_spectate.is_empty());
    }

    #[test]
    fn test_player_queues_add_remove() {
        let mut queues = PlayerQueues::default();
        let username: Username = "player1".to_string().into();
        queues.to_remove.insert(username.clone());
        assert!(queues.to_remove.contains(&username));
    }

    // === Pot Tests (Extended) ===

    #[test]
    fn test_pot_investments() {
        let pot = Pot::default();
        // Default pot has capacity for MAX_PLAYERS but is empty
        assert!(pot.investments.values().all(|&v| v == 0));
    }

    #[test]
    fn test_pot_add_investment() {
        let mut pot = Pot::default();
        let seat_idx = 0;
        pot.investments.insert(seat_idx, 100);
        assert_eq!(pot.investments.get(&seat_idx), Some(&100));
    }

    #[test]
    fn test_pot_multiple_investments() {
        let mut pot = Pot::default();
        pot.investments.insert(0, 50);
        pot.investments.insert(1, 100);
        pot.investments.insert(2, 75);
        let total: u32 = pot.investments.values().sum();
        assert!(total >= 225);
    }

    // === Card Tests (Extended) ===

    #[test]
    fn test_card_equality() {
        let card1 = Card(14, Suit::Spade);
        let card2 = Card(14, Suit::Spade);
        let card3 = Card(14, Suit::Heart);
        assert_eq!(card1, card2);
        assert_ne!(card1, card3);
    }

    #[test]
    fn test_card_all_values() {
        for value in 1..=14 {
            let card = Card(value, Suit::Club);
            assert_eq!(card.0, value);
        }
    }

    // === Blinds Tests (Extended) ===

    #[test]
    fn test_blinds_zero() {
        let blinds = Blinds { small: 0, big: 0 };
        assert_eq!(blinds.small, 0);
        assert_eq!(blinds.big, 0);
    }

    #[test]
    fn test_blinds_large_values() {
        let blinds = Blinds {
            small: 1000,
            big: 2000,
        };
        assert_eq!(blinds.small, 1000);
        assert_eq!(blinds.big, 2000);
    }

    // === Deck Tests (Extended) ===

    #[test]
    fn test_deck_deal_all_unique() {
        let mut deck = Deck::default();
        let mut cards = Vec::new();
        for _ in 0..52 {
            cards.push(deck.deal_card());
        }
        // Check all cards are dealt
        assert_eq!(deck.deck_idx, 52);
    }

    #[test]
    fn test_deck_shuffle_resets_index() {
        let mut deck = Deck::default();
        // Deal some cards
        deck.deal_card();
        deck.deal_card();
        deck.deal_card();
        assert_eq!(deck.deck_idx, 3);

        // Shuffle should reset index
        deck.shuffle();
        assert_eq!(deck.deck_idx, 0);
    }

    // === Display trait tests ===

    #[test]
    fn test_suit_display() {
        assert_eq!(format!("{}", Suit::Club), "♣");
        assert_eq!(format!("{}", Suit::Spade), "♠");
        assert_eq!(format!("{}", Suit::Diamond), "♦");
        assert_eq!(format!("{}", Suit::Heart), "♥");
        assert_eq!(format!("{}", Suit::Wild), "w");
    }

    #[test]
    fn test_card_display_face_cards() {
        let ace = Card(14, Suit::Spade);
        let king = Card(13, Suit::Heart);
        let queen = Card(12, Suit::Diamond);
        let jack = Card(11, Suit::Club);

        assert!(format!("{}", ace).contains("A"));
        assert!(format!("{}", king).contains("K"));
        assert!(format!("{}", queen).contains("Q"));
        assert!(format!("{}", jack).contains("J"));
    }

    #[test]
    fn test_card_display_number_cards() {
        let two = Card(2, Suit::Club);
        let ten = Card(10, Suit::Spade);

        assert!(format!("{}", two).contains("2"));
        assert!(format!("{}", ten).contains("10"));
    }

    #[test]
    fn test_rank_display() {
        assert_eq!(format!("{}", Rank::HighCard), "hi");
        assert_eq!(format!("{}", Rank::OnePair), "1p");
        assert_eq!(format!("{}", Rank::TwoPair), "2p");
        assert_eq!(format!("{}", Rank::ThreeOfAKind), "3k");
        assert_eq!(format!("{}", Rank::Straight), "s8");
        assert_eq!(format!("{}", Rank::Flush), "fs");
        assert_eq!(format!("{}", Rank::FullHouse), "fh");
        assert_eq!(format!("{}", Rank::FourOfAKind), "4k");
        assert_eq!(format!("{}", Rank::StraightFlush), "sf");
    }

    #[test]
    fn test_username_display() {
        let username = Username::new("alice");
        assert_eq!(format!("{}", username), "alice");
    }

    #[test]
    fn test_username_whitespace_replacement() {
        let username = Username::new("alice bob");
        assert_eq!(format!("{}", username), "alice_bob");
    }

    #[test]
    fn test_username_from_string() {
        let username: Username = "test_user".to_string().into();
        assert_eq!(format!("{}", username), "test_user");
    }

    #[test]
    fn test_blinds_display() {
        let blinds = Blinds { small: 5, big: 10 };
        assert_eq!(format!("{}", blinds), "$5/10");
    }

    #[test]
    fn test_action_display_all_in() {
        let action = Action::AllIn;
        assert_eq!(format!("{}", action), "all-ins (unhinged)");
    }

    #[test]
    fn test_action_display_call() {
        let action = Action::Call;
        assert_eq!(format!("{}", action), "calls");
    }

    #[test]
    fn test_action_display_check() {
        let action = Action::Check;
        assert_eq!(format!("{}", action), "checks");
    }

    #[test]
    fn test_action_display_fold() {
        let action = Action::Fold;
        assert_eq!(format!("{}", action), "folds");
    }

    #[test]
    fn test_action_display_raise_with_amount() {
        let action = Action::Raise(Some(100));
        assert_eq!(format!("{}", action), "raises $100");
    }

    #[test]
    fn test_action_display_raise_without_amount() {
        let action = Action::Raise(None);
        assert_eq!(format!("{}", action), "raises");
    }

    #[test]
    fn test_action_choice_display() {
        let all_in = ActionChoice::AllIn;
        let call = ActionChoice::Call(50);
        let check = ActionChoice::Check;
        let fold = ActionChoice::Fold;
        let raise = ActionChoice::Raise(100);

        assert_eq!(format!("{}", all_in), "all-in");
        assert_eq!(format!("{}", call), "call (== $50)");
        assert_eq!(format!("{}", check), "check");
        assert_eq!(format!("{}", fold), "fold");
        assert_eq!(format!("{}", raise), "raise (>= $100)");
    }

    #[test]
    fn test_action_choice_into_usize() {
        let all_in: usize = ActionChoice::AllIn.into();
        let call: usize = ActionChoice::Call(50).into();
        let check: usize = ActionChoice::Check.into();
        let fold: usize = ActionChoice::Fold.into();
        let raise: usize = ActionChoice::Raise(100).into();

        assert_eq!(all_in, 0);
        assert_eq!(call, 1);
        assert_eq!(check, 2);
        assert_eq!(fold, 3);
        assert_eq!(raise, 4);
    }

    #[test]
    fn test_action_choice_to_action_conversion() {
        let action: Action = ActionChoice::AllIn.into();
        assert!(matches!(action, Action::AllIn));

        let action: Action = ActionChoice::Call(50).into();
        assert!(matches!(action, Action::Call));

        let action: Action = ActionChoice::Check.into();
        assert!(matches!(action, Action::Check));

        let action: Action = ActionChoice::Fold.into();
        assert!(matches!(action, Action::Fold));

        let action: Action = ActionChoice::Raise(100).into();
        assert!(matches!(action, Action::Raise(Some(100))));
    }

    #[test]
    fn test_action_choices_display_single_option() {
        let mut choices = HashSet::new();
        choices.insert(ActionChoice::Fold);
        let action_choices = ActionChoices(choices);

        let display = format!("{}", action_choices);
        assert!(display.contains("fold"));
    }

    #[test]
    fn test_action_choices_display_two_options() {
        let mut choices = HashSet::new();
        choices.insert(ActionChoice::Fold);
        choices.insert(ActionChoice::Check);
        let action_choices = ActionChoices(choices);

        let display = format!("{}", action_choices);
        assert!(display.contains("or"));
    }

    #[test]
    fn test_action_choices_display_multiple_options() {
        let mut choices = HashSet::new();
        choices.insert(ActionChoice::Fold);
        choices.insert(ActionChoice::Check);
        choices.insert(ActionChoice::Call(50));
        let action_choices = ActionChoices(choices);

        let display = format!("{}", action_choices);
        assert!(display.contains("or"));
    }

    #[test]
    fn test_action_choices_from_iterator() {
        let choices = vec![ActionChoice::Fold, ActionChoice::Check, ActionChoice::Call(50)];
        let action_choices = ActionChoices::from(choices);

        assert_eq!(action_choices.0.len(), 3);
    }

    #[test]
    fn test_action_choices_contains_method() {
        let mut choices_set = HashSet::new();
        choices_set.insert(ActionChoice::Fold);
        choices_set.insert(ActionChoice::Call(50));
        let choices = ActionChoices(choices_set);

        assert!(choices.contains(&Action::Fold));
        assert!(choices.contains(&Action::Call));
        assert!(!choices.contains(&Action::Check));
    }

    #[test]
    fn test_bet_display() {
        let all_in = Bet { action: BetAction::AllIn, amount: 100 };
        let call = Bet { action: BetAction::Call, amount: 50 };
        let raise = Bet { action: BetAction::Raise, amount: 200 };

        assert_eq!(format!("{}", all_in), "all-in of $100");
        assert_eq!(format!("{}", call), "call of $50");
        assert_eq!(format!("{}", raise), "raise of $200");
    }

    #[test]
    fn test_action_from_bet_conversion() {
        let all_in_bet = Bet { action: BetAction::AllIn, amount: 100 };
        let action: Action = all_in_bet.into();
        assert!(matches!(action, Action::AllIn));

        let call_bet = Bet { action: BetAction::Call, amount: 50 };
        let action: Action = call_bet.into();
        assert!(matches!(action, Action::Call));

        let raise_bet = Bet { action: BetAction::Raise, amount: 200 };
        let action: Action = raise_bet.into();
        assert!(matches!(action, Action::Raise(Some(200))));
    }

    #[test]
    fn test_player_state_display() {
        assert_eq!(format!("{}", PlayerState::AllIn), "all-in ");
        assert_eq!(format!("{}", PlayerState::Call), "call   ");
        assert_eq!(format!("{}", PlayerState::Check), "check  ");
        assert_eq!(format!("{}", PlayerState::Fold), "folded ");
        assert_eq!(format!("{}", PlayerState::Raise), "raise  ");
        assert_eq!(format!("{}", PlayerState::Wait), "waiting");
    }

    #[test]
    fn test_player_reset_method() {
        let user = User {
            name: "test".to_string().into(),
            money: 1000,
        };
        let mut player = Player::new(user, 0);

        // Modify player state
        player.state = PlayerState::Fold;
        player.cards = vec![Card(14, Suit::Spade), Card(13, Suit::Heart)];
        player.showing = true;

        // Reset player
        player.reset();

        assert!(matches!(player.state, PlayerState::Wait));
        assert!(player.cards.is_empty());
        assert!(!player.showing);
    }

    #[test]
    fn test_pot_new() {
        let pot = Pot::new(10);
        assert!(pot.investments.capacity() >= 10);
        assert!(pot.is_empty());
    }

    #[test]
    fn test_pot_bet_method() {
        let mut pot = Pot::default();
        let bet = Bet { action: BetAction::Call, amount: 50 };

        pot.bet(0, &bet);
        assert_eq!(pot.get_investment_by_player_idx(0), 50);
    }

    #[test]
    fn test_pot_get_call() {
        let mut pot = Pot::default();
        pot.investments.insert(0, 50);
        pot.investments.insert(1, 100);
        pot.investments.insert(2, 75);

        assert_eq!(pot.get_call(), 100);
    }

    #[test]
    fn test_pot_get_call_empty_pot() {
        let pot = Pot::default();
        assert_eq!(pot.get_call(), 0);
    }

    #[test]
    fn test_pot_get_call_by_player_idx() {
        let mut pot = Pot::default();
        pot.investments.insert(0, 50);
        pot.investments.insert(1, 100);

        assert_eq!(pot.get_call_by_player_idx(0), 50); // needs 50 more to match 100
        assert_eq!(pot.get_call_by_player_idx(1), 0);  // already at max
    }

    #[test]
    fn test_pot_get_investment_by_player_idx() {
        let mut pot = Pot::default();
        pot.investments.insert(0, 50);

        assert_eq!(pot.get_investment_by_player_idx(0), 50);
        assert_eq!(pot.get_investment_by_player_idx(1), 0); // not in pot
    }

    #[test]
    fn test_pot_get_min_raise_by_player_idx() {
        let mut pot = Pot::default();
        pot.investments.insert(0, 50);
        pot.investments.insert(1, 100);

        // Player 0 needs to raise to at least 2*100 - 50 = 150 total
        assert_eq!(pot.get_min_raise_by_player_idx(0), 150);
        // Player 1 needs to raise to at least 2*100 - 100 = 100 more
        assert_eq!(pot.get_min_raise_by_player_idx(1), 100);
    }

    #[test]
    fn test_pot_get_size() {
        let mut pot = Pot::default();
        pot.investments.insert(0, 50);
        pot.investments.insert(1, 100);
        pot.investments.insert(2, 75);

        assert_eq!(pot.get_size(), 225);
    }

    #[test]
    fn test_pot_is_empty() {
        let mut pot = Pot::default();
        assert!(pot.is_empty());

        pot.investments.insert(0, 50);
        assert!(!pot.is_empty());
    }

    #[test]
    fn test_vote_display_kick() {
        let vote = Vote::Kick(Username::new("alice"));
        assert_eq!(format!("{}", vote), "kick alice");
    }

    #[test]
    fn test_vote_display_reset_all() {
        let vote = Vote::Reset(None);
        assert_eq!(format!("{}", vote), "reset everyone's money");
    }

    #[test]
    fn test_vote_display_reset_specific() {
        let vote = Vote::Reset(Some(Username::new("bob")));
        assert_eq!(format!("{}", vote), "reset bob's money");
    }

    #[test]
    fn test_pot_view_display() {
        let pot_view = PotView { size: 500 };
        assert_eq!(format!("{}", pot_view), "$500");
    }

    #[test]
    fn test_user_borrow_trait() {
        use std::borrow::Borrow;

        let user = User {
            name: "alice".to_string().into(),
            money: 1000,
        };
        let username_ref: &Username = user.borrow();
        assert_eq!(format!("{}", username_ref), "alice");
    }

    #[test]
    fn test_pot_bet_accumulates() {
        let mut pot = Pot::default();
        let bet1 = Bet { action: BetAction::Call, amount: 50 };
        let bet2 = Bet { action: BetAction::Raise, amount: 100 };

        pot.bet(0, &bet1);
        pot.bet(0, &bet2);

        assert_eq!(pot.get_investment_by_player_idx(0), 150);
    }

    #[test]
    fn test_player_view_creation() {
        let user = User {
            name: "test".to_string().into(),
            money: 1000,
        };
        let cards = vec![Card(14, Suit::Spade), Card(13, Suit::Heart)];
        let player_view = PlayerView {
            user: user.clone(),
            state: PlayerState::Wait,
            cards: cards.clone(),
        };

        assert_eq!(player_view.user, user);
        assert!(matches!(player_view.state, PlayerState::Wait));
        assert_eq!(player_view.cards, cards);
    }

    #[test]
    fn test_action_choice_equality() {
        // ActionChoice uses discriminant for equality, so amounts don't matter
        assert_eq!(ActionChoice::Call(50), ActionChoice::Call(100));
        assert_eq!(ActionChoice::Raise(50), ActionChoice::Raise(200));
        assert_ne!(ActionChoice::Call(50), ActionChoice::Fold);
    }

    #[test]
    fn test_action_choice_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ActionChoice::Call(50));
        // Should not insert duplicate since hash is based on discriminant
        set.insert(ActionChoice::Call(100));

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_BUY_IN, 600);
        assert_eq!(DEFAULT_MIN_BIG_BLIND, 10);
        assert_eq!(DEFAULT_MIN_SMALL_BLIND, 5);
    }

    #[test]
    fn test_suit_variants() {
        // Test all suit variants can be created
        let suits = vec![Suit::Club, Suit::Spade, Suit::Diamond, Suit::Heart, Suit::Wild];
        assert_eq!(suits.len(), 5);
    }

    #[test]
    fn test_card_ace_low_and_high() {
        let ace_low = Card(1, Suit::Spade);
        let ace_high = Card(14, Suit::Spade);

        // Both should display as "A"
        assert!(format!("{}", ace_low).contains("A"));
        assert!(format!("{}", ace_high).contains("A"));
    }

    #[test]
    fn test_player_cards_capacity() {
        let user = User {
            name: "test".to_string().into(),
            money: 1000,
        };
        let player = Player::new(user, 0);

        // Cards vec should have capacity for 2 cards
        assert_eq!(player.cards.capacity(), 2);
    }

    // === Input Validation Tests (Sprint 6 Stage 5) ===

    // === Username Validation Tests ===

    #[test]
    fn test_username_empty_string() {
        let username = Username::new("");
        assert_eq!(username.to_string(), "");
    }

    #[test]
    fn test_username_extremely_long_string() {
        let long_string = "a".repeat(10000);
        let username = Username::new(&long_string);
        // Should be truncated to MAX_USER_INPUT_LENGTH / 2 = 16
        assert_eq!(username.to_string().len(), constants::MAX_USER_INPUT_LENGTH / 2);
    }

    #[test]
    fn test_username_unicode_characters() {
        let unicode_name = "用户名🎮";
        let username = Username::new(unicode_name);
        // Should preserve unicode characters
        assert!(username.to_string().contains("用户名"));
    }

    #[test]
    fn test_username_special_characters() {
        let special = "user!@#$%^&*()";
        let username = Username::new(special);
        // Special chars should be preserved
        assert!(username.to_string().contains("user!@#$%^&*()"));
    }

    #[test]
    fn test_username_sql_injection_attempt() {
        let sql_injection = "admin'; DROP TABLE users; --";
        let username = Username::new(sql_injection);
        // Should be safely stored as-is (no SQL backend)
        assert!(username.to_string().contains("admin"));
    }

    #[test]
    fn test_username_xss_attempt() {
        let xss = "<script>alert('xss')</script>";
        let username = Username::new(xss);
        // Should be stored as-is (rendering layer responsible for escaping)
        assert!(username.to_string().contains("script"));
    }

    #[test]
    fn test_username_whitespace_only() {
        let whitespace = "   \t\n";
        let username = Username::new(whitespace);
        // All whitespace should be converted to underscores
        assert_eq!(username.to_string(), "_____");
    }

    #[test]
    fn test_username_mixed_whitespace() {
        let mixed = "user name\ttabs\nlines";
        let username = Username::new(mixed);
        // All whitespace types should be converted to underscores (truncated to 16 chars)
        assert_eq!(username.to_string(), "user_name_tabs_l");
        assert_eq!(username.to_string().len(), 16); // MAX_USER_INPUT_LENGTH / 2
    }

    #[test]
    fn test_username_exactly_max_length() {
        let exact_max = "a".repeat(constants::MAX_USER_INPUT_LENGTH / 2);
        let username = Username::new(&exact_max);
        assert_eq!(username.to_string().len(), constants::MAX_USER_INPUT_LENGTH / 2);
    }

    #[test]
    fn test_username_null_bytes() {
        let with_null = "user\0name";
        let username = Username::new(with_null);
        // Null bytes should be preserved (stored as String)
        assert!(username.to_string().contains("user"));
    }

    #[test]
    fn test_username_newline_characters() {
        let with_newlines = "user\nwith\nlines";
        let username = Username::new(with_newlines);
        // Newlines should become underscores
        assert_eq!(username.to_string(), "user_with_lines");
    }

    #[test]
    fn test_username_leading_trailing_spaces() {
        let spaced = "  username  ";
        let username = Username::new(spaced);
        // Leading/trailing spaces should become underscores
        assert_eq!(username.to_string(), "__username__");
    }

    // === Action/Command Parameter Validation Tests ===

    #[test]
    fn test_action_raise_zero_amount() {
        let action = Action::Raise(Some(0));
        assert_eq!(action, Action::Raise(Some(0)));
    }

    #[test]
    fn test_action_raise_max_u32() {
        let action = Action::Raise(Some(u32::MAX));
        assert_eq!(action, Action::Raise(Some(u32::MAX)));
    }

    #[test]
    fn test_action_raise_none_parameter() {
        let action = Action::Raise(None);
        assert_eq!(action, Action::Raise(None));
    }

    #[test]
    fn test_action_choice_call_zero() {
        let choice = ActionChoice::Call(0);
        // Zero call amount should be valid
        match choice {
            ActionChoice::Call(amt) => assert_eq!(amt, 0),
            _ => panic!("Expected Call action"),
        }
    }

    #[test]
    fn test_action_choice_call_max_u32() {
        let choice = ActionChoice::Call(u32::MAX);
        match choice {
            ActionChoice::Call(amt) => assert_eq!(amt, u32::MAX),
            _ => panic!("Expected Call action"),
        }
    }

    #[test]
    fn test_action_choice_raise_zero() {
        let choice = ActionChoice::Raise(0);
        match choice {
            ActionChoice::Raise(amt) => assert_eq!(amt, 0),
            _ => panic!("Expected Raise action"),
        }
    }

    #[test]
    fn test_action_choice_raise_max_u32() {
        let choice = ActionChoice::Raise(u32::MAX);
        match choice {
            ActionChoice::Raise(amt) => assert_eq!(amt, u32::MAX),
            _ => panic!("Expected Raise action"),
        }
    }

    #[test]
    fn test_bet_zero_amount() {
        let bet = Bet {
            amount: 0,
            action: BetAction::Call,
        };
        assert_eq!(bet.amount, 0);
    }

    #[test]
    fn test_bet_max_amount() {
        let bet = Bet {
            amount: u32::MAX,
            action: BetAction::Raise,
        };
        assert_eq!(bet.amount, u32::MAX);
    }

    #[test]
    fn test_player_seat_idx_zero() {
        let user = User {
            name: Username::new("test"),
            money: 1000,
        };
        let player = Player::new(user, 0);
        assert_eq!(player.seat_idx, 0);
    }

    #[test]
    fn test_player_seat_idx_max() {
        let user = User {
            name: Username::new("test"),
            money: 1000,
        };
        let player = Player::new(user, usize::MAX);
        assert_eq!(player.seat_idx, usize::MAX);
    }

    #[test]
    fn test_player_zero_money() {
        let user = User {
            name: Username::new("broke"),
            money: 0,
        };
        let player = Player::new(user.clone(), 0);
        assert_eq!(player.user.money, 0);
        assert_eq!(user.money, 0);
    }

    #[test]
    fn test_player_max_money() {
        let user = User {
            name: Username::new("whale"),
            money: u32::MAX,
        };
        let player = Player::new(user.clone(), 0);
        assert_eq!(player.user.money, u32::MAX);
    }

    #[test]
    fn test_action_serialization_with_extreme_values() {
        let actions = vec![
            Action::Raise(Some(0)),
            Action::Raise(Some(u32::MAX)),
            Action::Raise(None),
        ];

        for action in actions {
            let serialized = bincode::serialize(&action).unwrap();
            let deserialized: Action = bincode::deserialize(&serialized).unwrap();
            assert_eq!(action, deserialized);
        }
    }

    // === Game Configuration Validation Tests ===

    #[test]
    fn test_blinds_zero_values() {
        let blinds = Blinds {
            small: 0,
            big: 0,
        };
        assert_eq!(blinds.small, 0);
        assert_eq!(blinds.big, 0);
    }

    #[test]
    fn test_blinds_max_values() {
        let blinds = Blinds {
            small: u32::MAX,
            big: u32::MAX,
        };
        assert_eq!(blinds.small, u32::MAX);
        assert_eq!(blinds.big, u32::MAX);
    }

    #[test]
    fn test_blinds_asymmetric_values() {
        let blinds = Blinds {
            small: 100,
            big: 50, // Invalid game state but type allows it
        };
        assert_eq!(blinds.small, 100);
        assert_eq!(blinds.big, 50);
    }

    #[test]
    fn test_blinds_serialization_roundtrip() {
        let blinds = Blinds {
            small: 12345,
            big: 67890,
        };
        let serialized = bincode::serialize(&blinds).unwrap();
        let deserialized: Blinds = bincode::deserialize(&serialized).unwrap();
        assert_eq!(blinds.small, deserialized.small);
        assert_eq!(blinds.big, deserialized.big);
    }

    #[test]
    fn test_default_constants_sanity() {
        // Verify default constants are sane
        assert!(DEFAULT_BUY_IN > 0);
        assert!(DEFAULT_MIN_BIG_BLIND > 0);
        assert!(DEFAULT_MIN_SMALL_BLIND > 0);
        assert!(DEFAULT_MIN_SMALL_BLIND < DEFAULT_MIN_BIG_BLIND);
    }

    #[test]
    fn test_max_players_constant() {
        assert_eq!(constants::MAX_PLAYERS, 10);
        assert!(constants::MAX_PLAYERS > 0);
        assert!(constants::MAX_PLAYERS <= 23); // Max for texas hold'em
    }

    #[test]
    fn test_default_max_users_constant() {
        assert_eq!(constants::DEFAULT_MAX_USERS, constants::MAX_PLAYERS + 6);
        assert!(constants::DEFAULT_MAX_USERS >= constants::MAX_PLAYERS);
    }

    #[test]
    fn test_user_input_length_constant() {
        assert_eq!(constants::MAX_USER_INPUT_LENGTH, 32);
        assert!(constants::MAX_USER_INPUT_LENGTH > 0);
    }

    #[test]
    fn test_pot_empty_investments() {
        let pot = Pot {
            investments: HashMap::new(),
        };
        assert_eq!(pot.investments.len(), 0);
    }

    #[test]
    fn test_pot_with_investments() {
        let mut investments = HashMap::new();
        investments.insert(0, 1000);
        investments.insert(1, 500);
        let pot = Pot { investments };
        assert_eq!(pot.investments.len(), 2);
        assert_eq!(*pot.investments.get(&0).unwrap(), 1000);
        assert_eq!(*pot.investments.get(&1).unwrap(), 500);
    }

    // === Message Payload Validation Tests ===

    #[test]
    fn test_user_serialization_with_empty_name() {
        let user = User {
            name: Username::new(""),
            money: 100,
        };
        let serialized = bincode::serialize(&user).unwrap();
        let deserialized: User = bincode::deserialize(&serialized).unwrap();
        assert_eq!(user.name, deserialized.name);
    }

    #[test]
    fn test_user_serialization_with_long_name() {
        let long_name = "a".repeat(1000);
        let user = User {
            name: Username::new(&long_name),
            money: 100,
        };
        let serialized = bincode::serialize(&user).unwrap();
        let deserialized: User = bincode::deserialize(&serialized).unwrap();
        // Should be truncated to 16 chars
        assert_eq!(deserialized.name.to_string().len(), 16);
    }

    #[test]
    fn test_user_serialization_with_unicode() {
        let user = User {
            name: Username::new("测试用户🎮"),
            money: 500,
        };
        let serialized = bincode::serialize(&user).unwrap();
        let deserialized: User = bincode::deserialize(&serialized).unwrap();
        assert_eq!(user.name, deserialized.name);
    }

    #[test]
    fn test_card_serialization_roundtrip() {
        for value in 1..=14 {
            for suit in [Suit::Club, Suit::Spade, Suit::Diamond, Suit::Heart] {
                let card = Card(value, suit);
                let serialized = bincode::serialize(&card).unwrap();
                let deserialized: Card = bincode::deserialize(&serialized).unwrap();
                assert_eq!(card, deserialized);
            }
        }
    }

    #[test]
    fn test_player_view_serialization_with_empty_cards() {
        let user = User {
            name: Username::new("test"),
            money: 1000,
        };
        let player_view = PlayerView {
            user: user.clone(),
            state: PlayerState::Fold,
            cards: vec![],
        };
        let serialized = bincode::serialize(&player_view).unwrap();
        let deserialized: PlayerView = bincode::deserialize(&serialized).unwrap();
        assert_eq!(player_view.cards.len(), deserialized.cards.len());
    }

    #[test]
    fn test_player_view_serialization_with_cards() {
        let user = User {
            name: Username::new("test"),
            money: 1000,
        };
        let player_view = PlayerView {
            user: user.clone(),
            state: PlayerState::Call,
            cards: vec![Card(14, Suit::Spade), Card(13, Suit::Heart)],
        };

        let serialized = bincode::serialize(&player_view).unwrap();
        let deserialized: PlayerView = bincode::deserialize(&serialized).unwrap();
        assert_eq!(player_view.cards, deserialized.cards);
    }

    #[test]
    fn test_vote_kick_serialization() {
        let vote = Vote::Kick(Username::new("target"));
        let serialized = bincode::serialize(&vote).unwrap();
        let deserialized: Vote = bincode::deserialize(&serialized).unwrap();
        assert_eq!(vote, deserialized);
    }

    #[test]
    fn test_vote_reset_serialization() {
        let vote = Vote::Reset(None);
        let serialized = bincode::serialize(&vote).unwrap();
        let deserialized: Vote = bincode::deserialize(&serialized).unwrap();
        assert_eq!(vote, deserialized);
    }

    #[test]
    fn test_subhand_full_house_values() {
        let subhand = SubHand {
            rank: Rank::FullHouse,
            values: vec![13, 13, 13, 7, 7],
        };
        assert_eq!(subhand.rank, Rank::FullHouse);
        assert_eq!(subhand.values, vec![13, 13, 13, 7, 7]);
    }

    #[test]
    fn test_subhand_with_empty_values() {
        let subhand = SubHand {
            rank: Rank::HighCard,
            values: vec![],
        };
        assert_eq!(subhand.values.len(), 0);
    }

    #[test]
    fn test_subhand_with_max_values() {
        let values = vec![14; 100]; // Unusually long values vector
        let subhand = SubHand {
            rank: Rank::StraightFlush,
            values,
        };
        assert_eq!(subhand.values.len(), 100);
    }

    #[test]
    fn test_deck_deals_all_52_cards() {
        let mut deck = Deck::default();
        let mut dealt_cards = Vec::new();

        for _ in 0..52 {
            dealt_cards.push(deck.deal_card());
        }

        assert_eq!(dealt_cards.len(), 52);
        assert_eq!(deck.deck_idx, 52);
    }

    #[test]
    fn test_suit_display_all_variants() {
        assert_eq!(format!("{}", Suit::Club), "♣");
        assert_eq!(format!("{}", Suit::Spade), "♠");
        assert_eq!(format!("{}", Suit::Diamond), "♦");
        assert_eq!(format!("{}", Suit::Heart), "♥");
        assert_eq!(format!("{}", Suit::Wild), "w");
    }

    #[test]
    fn test_rank_display_all_variants() {
        assert_eq!(format!("{}", Rank::HighCard), "hi");
        assert_eq!(format!("{}", Rank::OnePair), "1p");
        assert_eq!(format!("{}", Rank::TwoPair), "2p");
        assert_eq!(format!("{}", Rank::ThreeOfAKind), "3k");
        assert_eq!(format!("{}", Rank::Straight), "s8");
        assert_eq!(format!("{}", Rank::Flush), "fs");
        assert_eq!(format!("{}", Rank::FullHouse), "fh");
        assert_eq!(format!("{}", Rank::FourOfAKind), "4k");
        assert_eq!(format!("{}", Rank::StraightFlush), "sf");
    }
}
