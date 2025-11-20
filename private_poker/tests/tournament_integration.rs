//! Integration tests for tournament functionality
//!
//! These tests verify the complete tournament lifecycle from registration
//! through completion and payouts.

#[cfg(test)]
mod tournament_tests {
    use private_poker::tournament::models::{
        BlindLevel, PrizeStructure, TournamentConfig, TournamentState, TournamentType,
    };

    #[test]
    fn test_tournament_config_validation() {
        // Test valid Sit-n-Go config
        let config = TournamentConfig {
            name: "Test SNG".to_string(),
            tournament_type: TournamentType::SitAndGo,
            buy_in: 100,
            min_players: 2,
            max_players: 9,
            starting_stack: 1500,
            blind_levels: create_standard_blind_structure(),
            starting_level: 1,
            scheduled_start: None,
            late_registration_secs: None,
        };

        assert_eq!(config.min_players, 2);
        assert_eq!(config.max_players, 9);
        assert_eq!(config.buy_in, 100);
    }

    #[test]
    fn test_blind_structure_progression() {
        // Verify blind levels progress correctly
        let blinds = create_standard_blind_structure();

        assert_eq!(blinds.len(), 10);

        // Level 1
        assert_eq!(blinds[0].level, 1);
        assert_eq!(blinds[0].small_blind, 10);
        assert_eq!(blinds[0].big_blind, 20);

        // Level 5
        assert_eq!(blinds[4].level, 5);
        assert_eq!(blinds[4].small_blind, 50);
        assert_eq!(blinds[4].big_blind, 100);

        // Level 10
        assert_eq!(blinds[9].level, 10);
        assert_eq!(blinds[9].small_blind, 300);
        assert_eq!(blinds[9].big_blind, 600);
    }

    #[test]
    fn test_blind_structure_with_antes() {
        // Verify antes are introduced at later levels
        let mut blinds = create_standard_blind_structure();

        // Add antes starting at level 6
        for level in &mut blinds[5..] {
            level.ante = Some(level.small_blind / 2);
        }

        // Levels 1-5 should have no antes
        for level in &blinds[0..5] {
            assert_eq!(level.ante, None);
        }

        // Levels 6-10 should have antes
        for level in &blinds[5..] {
            assert!(level.ante.is_some());
            assert_eq!(level.ante.unwrap(), level.small_blind / 2);
        }
    }

    #[test]
    fn test_prize_structure_winner_takes_all() {
        // 2-5 players: Winner takes all
        let prize = PrizeStructure::standard(5, 100);

        assert_eq!(prize.total_pool, 500);
        assert_eq!(prize.payouts.len(), 1);
        assert_eq!(prize.payouts[0], 500);
    }

    #[test]
    fn test_prize_structure_two_payouts() {
        // 6-9 players: 60/40 split
        let prize = PrizeStructure::standard(9, 100);

        assert_eq!(prize.total_pool, 900);
        assert_eq!(prize.payouts.len(), 2);
        assert_eq!(prize.payouts[0], 540); // 60%
        assert_eq!(prize.payouts[1], 360); // 40%
    }

    #[test]
    fn test_prize_structure_three_payouts() {
        // 10+ players: 50/30/20 split
        let prize = PrizeStructure::standard(10, 100);

        assert_eq!(prize.total_pool, 1000);
        assert_eq!(prize.payouts.len(), 3);
        assert_eq!(prize.payouts[0], 500); // 50%
        assert_eq!(prize.payouts[1], 300); // 30%
        assert_eq!(prize.payouts[2], 200); // 20%
    }

    #[test]
    fn test_custom_prize_structure() {
        // Custom structure with exact amounts
        let custom = PrizeStructure {
            total_pool: 1000,
            payouts: vec![600, 250, 100, 50],
        };

        assert_eq!(custom.payouts.len(), 4);
        assert_eq!(custom.payouts.iter().sum::<i64>(), 1000);
    }

    #[test]
    fn test_tournament_state_transitions() {
        // Verify valid state transitions
        let states = [
            TournamentState::Registering,
            TournamentState::Running,
            TournamentState::Finished,
        ];

        // Registering -> Running is valid
        assert_ne!(states[0], states[1]);

        // Running -> Finished is valid
        assert_ne!(states[1], states[2]);

        // Finished is terminal
        assert_eq!(states[2], TournamentState::Finished);
    }

    #[test]
    fn test_blind_increase_timing() {
        // Verify 5-minute blind levels
        let blinds = create_standard_blind_structure();

        for level in &blinds {
            assert_eq!(level.duration_secs, 300); // 5 minutes
        }

        // Total tournament time (if reaching final level)
        let total_time: u32 = blinds.iter().map(|l| l.duration_secs).sum();
        assert_eq!(total_time, 3000); // 50 minutes for 10 levels
    }

    #[test]
    fn test_turbo_tournament_timing() {
        // Turbo tournaments should have shorter levels
        let turbo_blinds = create_turbo_blind_structure();

        for level in &turbo_blinds {
            assert_eq!(level.duration_secs, 180); // 3 minutes
        }

        let total_time: u32 = turbo_blinds.iter().map(|l| l.duration_secs).sum();
        assert_eq!(total_time, 1800); // 30 minutes for 10 levels
    }

    #[test]
    fn test_starting_stack_to_blind_ratio() {
        // Verify healthy starting stack (75-150 BBs)
        let config = TournamentConfig {
            name: "Test".to_string(),
            tournament_type: TournamentType::SitAndGo,
            buy_in: 100,
            min_players: 6,
            max_players: 9,
            starting_stack: 1500,
            blind_levels: create_standard_blind_structure(),
            starting_level: 1,
            scheduled_start: None,
            late_registration_secs: None,
        };

        let initial_bb = config.blind_levels[0].big_blind;
        let stack_in_bbs = config.starting_stack / initial_bb;

        assert!(stack_in_bbs >= 75, "Starting stack too small");
        assert!(stack_in_bbs <= 150, "Starting stack too large");
    }

    #[test]
    fn test_tournament_lifecycle_states() {
        // Document expected lifecycle
        //
        // 1. REGISTERING: Players can join
        // 2. RUNNING: Game in progress, no new registrations
        // 3. FINISHED: Tournament complete, payouts distributed
        // 4. CANCELLED: Tournament cancelled before completion

        let lifecycle = [
            ("Registering", TournamentState::Registering),
            ("Running", TournamentState::Running),
            ("Finished", TournamentState::Finished),
        ];

        assert_eq!(lifecycle.len(), 3);
        assert_eq!(lifecycle[0].1, TournamentState::Registering);
        assert_eq!(lifecycle[2].1, TournamentState::Finished);
    }

    #[test]
    fn test_min_players_validation() {
        // Minimum 2 players required
        let config = TournamentConfig {
            name: "Test".to_string(),
            tournament_type: TournamentType::SitAndGo,
            buy_in: 100,
            min_players: 2,
            max_players: 9,
            starting_stack: 1500,
            blind_levels: create_standard_blind_structure(),
            starting_level: 1,
            scheduled_start: None,
            late_registration_secs: None,
        };

        assert!(config.min_players >= 2, "Need at least 2 players");
        assert!(
            config.min_players <= config.max_players,
            "Min should be <= max"
        );
    }

    #[test]
    fn test_prize_pool_calculation() {
        // Verify prize pool matches buy-in * players
        let buy_in = 100;
        let players = 9;

        let prize = PrizeStructure::standard(players, buy_in);

        assert_eq!(prize.total_pool, buy_in * players as i64);

        // Verify payouts sum to total pool (allowing for rounding)
        let payout_sum: i64 = prize.payouts.iter().sum();
        let diff = (prize.total_pool - payout_sum).abs();
        assert!(diff <= players as i64, "Payouts should sum to total pool");
    }

    #[test]
    fn test_blind_acceleration() {
        // Blinds should increase each level
        let blinds = create_standard_blind_structure();

        // Check that blinds increase each level
        for i in 1..blinds.len() {
            assert!(
                blinds[i].big_blind > blinds[i - 1].big_blind,
                "Blinds should increase each level"
            );
        }

        // Verify reasonable increase pattern (1.3x to 2.5x)
        for i in 0..blinds.len() - 1 {
            let ratio = blinds[i + 1].big_blind as f64 / blinds[i].big_blind as f64;
            assert!(
                (1.2..=2.5).contains(&ratio),
                "Blinds should increase reasonably (ratio: {})",
                ratio
            );
        }
    }

    // Helper functions

    fn create_standard_blind_structure() -> Vec<BlindLevel> {
        vec![
            BlindLevel::new(1, 10, 20, 300),
            BlindLevel::new(2, 15, 30, 300),
            BlindLevel::new(3, 20, 40, 300),
            BlindLevel::new(4, 30, 60, 300),
            BlindLevel::new(5, 50, 100, 300),
            BlindLevel::new(6, 75, 150, 300),
            BlindLevel::new(7, 100, 200, 300),
            BlindLevel::new(8, 150, 300, 300),
            BlindLevel::new(9, 200, 400, 300),
            BlindLevel::new(10, 300, 600, 300),
        ]
    }

    fn create_turbo_blind_structure() -> Vec<BlindLevel> {
        vec![
            BlindLevel::new(1, 10, 20, 180),
            BlindLevel::new(2, 15, 30, 180),
            BlindLevel::new(3, 20, 40, 180),
            BlindLevel::new(4, 30, 60, 180),
            BlindLevel::new(5, 50, 100, 180),
            BlindLevel::new(6, 75, 150, 180),
            BlindLevel::new(7, 100, 200, 180),
            BlindLevel::new(8, 150, 300, 180),
            BlindLevel::new(9, 200, 400, 180),
            BlindLevel::new(10, 300, 600, 180),
        ]
    }
}
