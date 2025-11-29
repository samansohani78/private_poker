//! Seat randomization to prevent seat selection manipulation.

use rand::seq::SliceRandom;
use std::collections::HashMap;

/// Seat randomizer for anti-collusion
pub struct SeatRandomizer {
    /// Random number generator
    rng: rand::rngs::ThreadRng,
}

impl SeatRandomizer {
    /// Create a new seat randomizer
    pub fn new() -> Self {
        Self { rng: rand::rng() }
    }

    /// Assign random seats to players
    ///
    /// # Arguments
    ///
    /// * `user_ids` - List of user IDs to seat
    /// * `max_seats` - Maximum number of seats at table
    ///
    /// # Returns
    ///
    /// * `HashMap<i64, usize>` - Map of user_id to seat index
    pub fn assign_seats(&mut self, user_ids: &[i64], max_seats: usize) -> HashMap<i64, usize> {
        if user_ids.is_empty() {
            return HashMap::new();
        }

        // Create list of available seat indices
        let mut available_seats: Vec<usize> = (0..max_seats).collect();
        available_seats.shuffle(&mut self.rng);

        // Assign random seat to each user
        let mut assignments = HashMap::new();
        for (idx, &user_id) in user_ids.iter().enumerate() {
            if idx < available_seats.len() {
                assignments.insert(user_id, available_seats[idx]);
            }
        }

        assignments
    }

    /// Find a random available seat
    ///
    /// # Arguments
    ///
    /// * `occupied_seats` - Set of currently occupied seat indices
    /// * `max_seats` - Maximum number of seats
    ///
    /// # Returns
    ///
    /// * `Option<usize>` - Random available seat index or None if table full
    pub fn find_random_seat(
        &mut self,
        occupied_seats: &[usize],
        max_seats: usize,
    ) -> Option<usize> {
        let mut available_seats: Vec<usize> = (0..max_seats)
            .filter(|seat| !occupied_seats.contains(seat))
            .collect();

        if available_seats.is_empty() {
            return None;
        }

        available_seats.shuffle(&mut self.rng);
        Some(available_seats[0])
    }

    /// Shuffle seat order for new hand (optional feature)
    ///
    /// # Arguments
    ///
    /// * `current_assignments` - Current seat assignments
    ///
    /// # Returns
    ///
    /// * `HashMap<i64, usize>` - New randomized seat assignments
    pub fn shuffle_seats(
        &mut self,
        current_assignments: &HashMap<i64, usize>,
    ) -> HashMap<i64, usize> {
        let user_ids: Vec<i64> = current_assignments.keys().copied().collect();
        // Calculate max seats based on highest assigned seat + 1, default to 10 if empty
        let max_seats = current_assignments
            .values()
            .max()
            .map(|&v| v + 1)
            .unwrap_or(10); // Safe: unwrap_or provides default value, cannot panic
        self.assign_seats(&user_ids, max_seats)
    }
}

impl Default for SeatRandomizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assign_seats_randomizes() {
        let mut randomizer = SeatRandomizer::new();
        let user_ids = vec![1, 2, 3, 4, 5];

        let assignments1 = randomizer.assign_seats(&user_ids, 10);
        let assignments2 = randomizer.assign_seats(&user_ids, 10);

        // All users should be assigned
        assert_eq!(assignments1.len(), 5);
        assert_eq!(assignments2.len(), 5);

        // Seats should be within range
        for &seat in assignments1.values() {
            assert!(seat < 10);
        }

        // High probability assignments are different (not guaranteed but very likely)
        // This tests randomization is working
        let same_count = user_ids
            .iter()
            .filter(|&&uid| assignments1.get(&uid) == assignments2.get(&uid))
            .count();
        assert!(same_count < 5, "Assignments should be randomized");
    }

    #[test]
    fn test_find_random_seat() {
        let mut randomizer = SeatRandomizer::new();
        let occupied = vec![0, 2, 4];

        let seat = randomizer.find_random_seat(&occupied, 10);
        assert!(seat.is_some());

        // Use expect() in tests for clearer failure messages
        let seat_idx = seat.expect("Should find available seat when table not full");
        assert!(!occupied.contains(&seat_idx));
        assert!(seat_idx < 10);
    }

    #[test]
    fn test_find_random_seat_full_table() {
        let mut randomizer = SeatRandomizer::new();
        let occupied = vec![0, 1, 2];

        let seat = randomizer.find_random_seat(&occupied, 3);
        assert!(seat.is_none());
    }

    #[test]
    fn test_empty_user_list() {
        let mut randomizer = SeatRandomizer::new();
        let assignments = randomizer.assign_seats(&[], 10);
        assert!(assignments.is_empty());
    }
}
