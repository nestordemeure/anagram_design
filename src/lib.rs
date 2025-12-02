// Module declarations
pub mod cost;
pub mod node;
pub mod constraints;
pub mod context;
pub mod dijkstra_solver;
pub mod format;
pub mod api;
pub mod merged;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-export public API
pub use cost::{Cost, compare_costs};
pub use node::{Node, NodeRef, Solution};
pub use format::format_tree;
pub use api::minimal_trees;
pub use merged::{MergedNode, MergedOption, NodeInfo};

// Re-export WASM bindings (they have their own #[wasm_bindgen] attributes)
#[cfg(target_arch = "wasm32")]
pub use wasm::{solve_words, zodiac_words};

#[cfg(test)]
mod tests
{
    use super::*;

    fn words(list: &[&str]) -> Vec<String>
    {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn compare_costs_prioritization_flips()
    {
        use std::cmp::Ordering;

        let soft_first = Cost { hard_nos: 0, redeemed_hard_nos: 0, nos: 2, redeemed_nos: 4, sum_hard_nos: 0, redeemed_sum_hard_nos: 0, sum_nos: 4, redeemed_sum_nos: 8, word_count: 4 };
        let hard_first = Cost { hard_nos: 1, redeemed_hard_nos: 2, nos: 1, redeemed_nos: 2, sum_hard_nos: 1, redeemed_sum_hard_nos: 2, sum_nos: 2, redeemed_sum_nos: 4, word_count: 4 };

        assert_eq!(compare_costs(&soft_first, &hard_first, true), Ordering::Less);
        assert_eq!(compare_costs(&soft_first, &hard_first, false), Ordering::Greater);
    }

    #[test]
    fn repeat_beats_depth_for_two_words()
    {
        use std::cmp::Ordering;
        let data = words(&["alpha", "beta"]);
        let with_repeat = minimal_trees(&data, true, true, 2);
        let without_repeat = minimal_trees(&data, false, true, 2);
        assert_eq!(compare_costs(&with_repeat.cost, &without_repeat.cost, true), Ordering::Less);
        assert!(matches!(&*with_repeat.trees[0], Node::Repeat { .. }));
    }

    #[test]
    fn simple_split_cost()
    {
        let data = words(&["ab", "ac", "b"]);
        let sol = minimal_trees(&data, false, true, 2);
        // Improved cost with better exception handling
        assert_eq!(sol.cost, Cost { hard_nos: 1, redeemed_hard_nos: 2, nos: 1, redeemed_nos: 2, sum_hard_nos: 1, redeemed_sum_hard_nos: 2, sum_nos: 2, redeemed_sum_nos: 4, word_count: 3 });
    }

    #[test]
    fn zodiac_costs_baseline()
    {
        // Baseline test with redeeming_yes=0 to ensure behavior stays fixed
        let data = words(&["aries",
                           "taurus",
                           "gemini",
                           "cancer",
                           "leo",
                           "virgo",
                           "libra",
                           "scorpio",
                           "sagittarius",
                           "capricorn",
                           "aquarius",
                           "pisces"]);
        let allow_repeat = minimal_trees(&data, true, true, 0);
        let no_repeat = minimal_trees(&data, false, true, 0);
        // With all 9 position types enabled, we achieve even better (lower) sum_hard_nos costs
        // by using more positional soft splits
        assert_eq!(allow_repeat.cost, Cost { hard_nos: 1,
                                             redeemed_hard_nos: 0,
                                             nos: 2,
                                             redeemed_nos: 0,
                                             sum_hard_nos: 3,
                                             redeemed_sum_hard_nos: 0,
                                             sum_nos: 14,
                                             redeemed_sum_nos: 0,
                                             word_count: 12 });
        // With the corrected collision detection (checking only NO branch),
        // we get better trees with improved sum_hard_nos
        assert_eq!(no_repeat.cost, Cost { hard_nos: 1,
                                          redeemed_hard_nos: 0,
                                          nos: 2,
                                          redeemed_nos: 0,
                                          sum_hard_nos: 5,
                                          redeemed_sum_hard_nos: 0,
                                          sum_nos: 17,
                                          redeemed_sum_nos: 0,
                                          word_count: 12 });
    }

    #[test]
    fn zodiac_costs()
    {
        // Test with redeeming_yes=2 (default)
        let data = words(&["aries",
                           "taurus",
                           "gemini",
                           "cancer",
                           "leo",
                           "virgo",
                           "libra",
                           "scorpio",
                           "sagittarius",
                           "capricorn",
                           "aquarius",
                           "pisces"]);
        let allow_repeat = minimal_trees(&data, true, true, 2);
        let no_repeat = minimal_trees(&data, false, true, 2);
        // With all 9 position types enabled, we achieve even better (lower) sum_hard_nos costs
        // by using more positional soft splits
        assert_eq!(allow_repeat.cost, Cost { hard_nos: 1,
                                             redeemed_hard_nos: 0,
                                             nos: 2,
                                             redeemed_nos: 4,
                                             sum_hard_nos: 4,
                                             redeemed_sum_hard_nos: 4,
                                             sum_nos: 14,
                                             redeemed_sum_nos: 22,
                                             word_count: 12 });
        // With the corrected collision detection (checking only NO branch)
        // AND proper YesSplit constraint propagation, we get better trees
        assert_eq!(no_repeat.cost, Cost { hard_nos: 1,
                                          redeemed_hard_nos: 0,
                                          nos: 2,
                                          redeemed_nos: 4,
                                          sum_hard_nos: 5,
                                          redeemed_sum_hard_nos: 8,
                                          sum_nos: 17,
                                          redeemed_sum_nos: 32,
                                          word_count: 12 });
    }

    #[test]
    fn virgo_scorpio_soft_separation()
    {
        // Verify that Virgo and Scorpio CAN be separated using only soft tests
        // This is possible with R/E and C/G pairs:
        //   virgo: has {v,i,r,g,o} - has 'r' and 'g', no 'c'
        //   scorpio: has {s,c,o,r,p,i} - has 'r' and 'c', no 'g'
        //   gemini: has {g,e,m,i,n} - has 'e' and 'g', no 'r' or 'c'
        let data = words(&["virgo", "scorpio", "gemini"]);
        let sol = minimal_trees(&data, true, true, 2);
        // Should achieve hard_nos: 0 using: r/e soft, then c/g soft
        assert_eq!(sol.cost.hard_nos, 0, "Expected 0 hard NOs (all soft), got {}", sol.cost.hard_nos);
    }

    #[test]
    fn soft_known_letter_pruning_regression()
    {
        // With improved exception handling, we can now achieve all-soft separation
        let data = words(&["tr", "r", "e"]);
        let sol = minimal_trees(&data, false, true, 2);
        assert_eq!(sol.cost,
                   Cost { hard_nos: 0, redeemed_hard_nos: 0, nos: 1, redeemed_nos: 2, sum_hard_nos: 0, redeemed_sum_hard_nos: 0, sum_nos: 2, redeemed_sum_nos: 4, word_count: 3 },
                   "Expected all-soft separation; got {:?}",
                   sol.cost);
    }

    #[test]
    fn soft_mirror_first_last_split_works()
    {
        // Front test, back requirement mirror keeps the miss soft
        let data = words(&["axe", "exa"]);
        let sol = minimal_trees(&data, false, true, 2);
        assert_eq!(sol.cost, Cost { hard_nos: 0, redeemed_hard_nos: 0, nos: 1, redeemed_nos: 2, sum_hard_nos: 0, redeemed_sum_hard_nos: 0, sum_nos: 1, redeemed_sum_nos: 2, word_count: 2 });
        match &*sol.trees[0]
        {
            Node::PositionalSplit { test_letter,
                                    test_position,
                                    requirement_letter,
                                    requirement_position,
                                    .. } =>
            {
                // Should be first 'a' with requirement last 'a' (mirror)
                assert_eq!(*test_letter, 'a');
                assert_eq!(*requirement_letter, 'a');
                assert_eq!(*test_position, node::Position::First);
                assert_eq!(*requirement_position, node::Position::Last);
            }
            other => panic!("expected PositionalSplit (first/last mirror) root, got {other:?}")
        }
    }

    #[test]
    fn test_position_collision_detection()
    {
        use node::Position;
        use constraints::positions_can_collide;

        // Second and Second-to-last collide for 3-letter words
        assert!(positions_can_collide(Position::Second, Position::SecondToLast),
                "Second and Second-to-last should collide for 3-letter words");

        // Third and Third-to-last collide for 5-letter words
        assert!(positions_can_collide(Position::Third, Position::ThirdToLast),
                "Third and Third-to-last should collide for 5-letter words");

        // First and Last collide for 1-letter words
        assert!(positions_can_collide(Position::First, Position::Last),
                "First and Last should collide for 1-letter words");

        // First and Second should never collide
        assert!(!positions_can_collide(Position::First, Position::Second),
                "First and Second should never collide");

        // Contains is not positional, so can't collide
        assert!(!positions_can_collide(Position::Contains, Position::First),
                "Contains should not collide with positional");
    }

    #[test]
    fn test_position_to_absolute_index()
    {
        use node::Position;

        // Test 3-letter word (e.g., "Leo")
        assert_eq!(Position::First.to_absolute_index(3), Some(0));
        assert_eq!(Position::Second.to_absolute_index(3), Some(1));
        assert_eq!(Position::Third.to_absolute_index(3), Some(2));
        assert_eq!(Position::Last.to_absolute_index(3), Some(2));
        assert_eq!(Position::SecondToLast.to_absolute_index(3), Some(1));
        assert_eq!(Position::ThirdToLast.to_absolute_index(3), Some(0));

        // Verify Second and SecondToLast map to same index for 3-letter words
        assert_eq!(Position::Second.to_absolute_index(3),
                   Position::SecondToLast.to_absolute_index(3),
                   "Second and SecondToLast should map to same index (1) for 3-letter words");

        // Test 5-letter word
        assert_eq!(Position::Third.to_absolute_index(5), Some(2));
        assert_eq!(Position::ThirdToLast.to_absolute_index(5), Some(2));

        // Contains/Double/Triple are not positional
        assert_eq!(Position::Contains.to_absolute_index(5), None);
        assert_eq!(Position::Double.to_absolute_index(5), None);
        assert_eq!(Position::Triple.to_absolute_index(5), None);
    }

    #[test]
    fn test_same_index_restriction_leo_gemini()
    {
        // Test with Leo and Gemini where E appears at different positions
        // Leo: 3 letters (l-e-o), E is at Second (index 1) and SecondToLast (index 1) - same!
        // Gemini: 6 letters (g-e-m-i-n-i), E is at Second (index 1), SecondToLast is 'n' (index 4)
        // The solver CAN use "Second E?" to separate them, and in the YES branch containing both,
        // it CAN use "SecondToLast E?" because in that context, only words where the positions
        // don't collide are relevant for the NO branch requirement.
        let data = words(&["leo", "gemini"]);
        let sol = minimal_trees(&data, false, true, 2);

        // Just verify we get a valid solution
        assert!(!sol.is_unsolvable());
        assert!(!sol.trees.is_empty());
    }

    #[test]
    fn test_same_index_restriction_zodiac_subset()
    {
        // Test with a subset of zodiac words (those without 'R')
        // With the fixed collision detection, we should get better (lower cost) trees
        // because we can now use soft splits like "Second E? (all No have I second)"
        // even when Leo is in the YES branch (collision doesn't matter there).
        let data = words(&["leo", "gemini", "pisces"]);
        let sol = minimal_trees(&data, false, true, 2);

        // Just verify we get a valid solution with reasonable cost
        assert!(!sol.is_unsolvable());
        assert!(!sol.trees.is_empty());
        // Cost should be quite low for just 3 words
        assert!(sol.cost.nos <= 2);
    }

    #[test]
    fn split_with_repeat_branches_after_fix()
    {
        // After fixing the memoization bug (adding allow_repeat to Key),
        // word sets that cleanly partition should use Split(yes: Repeat, no: Repeat)
        // instead of Repeat at the root.

        let data = words(&["bar", "car", "bee", "see"]);
        let sol = minimal_trees(&data, true, true, 2);

        println!("\nSolution for {{bar, car, bee, see}}:");
        println!("Cost: {:?}", sol.cost);
        println!("Tree:\n{}", format_tree(&sol.trees[0]));

        // After the fix, we expect:
        // - Root should be a Split (not Repeat)
        // - Both branches should be Repeat nodes
        // - Cost should be {hard_nos: 0, nos: 1, ...} (better than the old {hard_nos: 1, nos: 1, ...})

        match &*sol.trees[0]
        {
            Node::PositionalSplit { yes, no, .. } =>
            {
                let yes_is_repeat = matches!(&**yes, Node::Repeat { .. });
                let no_is_repeat = matches!(&**no, Node::Repeat { .. });

                assert!(yes_is_repeat, "Yes branch should be Repeat after fix");
                assert!(no_is_repeat, "No branch should be Repeat after fix");

                println!("\n✓ SUCCESS: Found Split(yes: Repeat, no: Repeat) pattern!");
            }
            _ =>
            {
                panic!("Root should be Split after fix, but got: {:?}", sol.trees[0]);
            }
        }

        // Verify the cost is better than before
        assert_eq!(sol.cost.hard_nos, 0, "Should have 0 hard_nos (all soft splits)");
        assert_eq!(sol.cost.nos, 1, "Should have 1 no edge");
    }
}
