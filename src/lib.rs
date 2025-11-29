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
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn leaves(node: &Node) -> Vec<String> {
        fn walk(node: &Node, out: &mut Vec<String>) {
            match node {
                Node::Leaf(w) => out.push(w.clone()),
                Node::Repeat { word, no } => {
                    out.push(word.clone());
                    walk(no, out);
                }
                Node::PositionalSplit { yes, no, .. } => {
                    walk(yes, out);
                    walk(no, out);
                }
            }
        }

        let mut out = Vec::new();
        walk(node, &mut out);
        out
    }

    #[test]
    fn compare_costs_prioritization_flips() {
        use std::cmp::Ordering;

        let soft_first = Cost {
            hard_nos: 0,
            nos: 2,
            sum_hard_nos: 0,
            sum_nos: 4,
            depth: 2,
            word_count: 4,
        };
        let hard_first = Cost {
            hard_nos: 1,
            nos: 1,
            sum_hard_nos: 1,
            sum_nos: 2,
            depth: 3,
            word_count: 4,
        };

        assert_eq!(
            compare_costs(&soft_first, &hard_first, true),
            Ordering::Less
        );
        assert_eq!(
            compare_costs(&soft_first, &hard_first, false),
            Ordering::Greater
        );
    }

    #[test]
    fn repeat_beats_depth_for_two_words() {
        let data = words(&["alpha", "beta"]);
        let with_repeat = minimal_trees(&data, true, true);
        let without_repeat = minimal_trees(&data, false, true);
        assert!(with_repeat.cost < without_repeat.cost);
        assert!(matches!(&*with_repeat.trees[0], Node::Repeat { .. }));
    }

    #[test]
    fn simple_split_cost() {
        let data = words(&["ab", "ac", "b"]);
        let sol = minimal_trees(&data, false, true);
        // Improved cost with better exception handling
        assert_eq!(
            sol.cost,
            Cost {
                nos: 1,
                hard_nos: 1,
                sum_nos: 2,
                sum_hard_nos: 1,
                depth: 2,
                word_count: 3
            }
        );
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries",
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
            "pisces",
        ]);
        let allow_repeat = minimal_trees(&data, true, true);
        let no_repeat = minimal_trees(&data, false, true);
        // With all 9 position types enabled, we achieve even better (lower) sum_hard_nos costs
        // by using more positional soft splits
        assert_eq!(
            allow_repeat.cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 15,
                sum_hard_nos: 3,
                depth: 6,
                word_count: 12
            }
        );
        // With same-index restriction, costs are slightly different
        assert_eq!(
            no_repeat.cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 16,
                sum_hard_nos: 6,
                depth: 6,
                word_count: 12
            }
        );
    }

    #[test]
    fn virgo_scorpio_soft_separation() {
        // Verify that Virgo and Scorpio CAN be separated using only soft tests
        // This is possible with R/E and C/G pairs:
        //   virgo: has {v,i,r,g,o} - has 'r' and 'g', no 'c'
        //   scorpio: has {s,c,o,r,p,i} - has 'r' and 'c', no 'g'
        //   gemini: has {g,e,m,i,n} - has 'e' and 'g', no 'r' or 'c'
        let data = words(&["virgo", "scorpio", "gemini"]);
        let sol = minimal_trees(&data, true, true);
        // Should achieve hard_nos: 0 using: r/e soft, then c/g soft
        assert_eq!(
            sol.cost.hard_nos, 0,
            "Expected 0 hard NOs (all soft), got {}",
            sol.cost.hard_nos
        );
    }

    #[test]
    fn soft_known_letter_pruning_regression() {
        // With improved exception handling, we can now achieve all-soft separation
        let data = words(&["tr", "r", "e"]);
        let sol = minimal_trees(&data, false, true);
        assert_eq!(
            sol.cost,
            Cost {
                hard_nos: 0,
                nos: 1,
                sum_hard_nos: 0,
                sum_nos: 2,
                depth: 2,
                word_count: 3
            },
            "Expected all-soft separation; got {:?}",
            sol.cost
        );
    }

    #[test]
    fn soft_double_letter_split_works() {
        // With all position types, the solver can find various valid solutions
        let data = words(&["book", "pool", "ball", "tall"]);
        let sol = minimal_trees(&data, false, true);

        // Check that we get a reasonable cost
        assert_eq!(
            sol.cost,
            Cost {
                nos: 1,
                hard_nos: 1,
                sum_nos: 3,
                sum_hard_nos: 2,
                depth: 3,
                word_count: 4
            }
        );

        // Verify all words are present in the tree
        let mut tree_leaves = leaves(&sol.trees[0]);
        tree_leaves.sort();
        assert_eq!(tree_leaves, vec!["ball".to_string(), "book".to_string(), "pool".to_string(), "tall".to_string()]);
    }

    #[test]
    fn soft_mirror_first_last_split_works() {
        // Front test, back requirement mirror keeps the miss soft
        let data = words(&["axe", "exa"]);
        let sol = minimal_trees(&data, false, true);
        assert_eq!(
            sol.cost,
            Cost {
                nos: 1,
                hard_nos: 0,
                sum_nos: 1,
                sum_hard_nos: 0,
                depth: 1,
                word_count: 2
            }
        );
        match &*sol.trees[0] {
            Node::PositionalSplit {
                test_letter,
                test_position,
                requirement_letter,
                requirement_position,
                ..
            } => {
                // Should be first 'a' with requirement last 'a' (mirror)
                assert_eq!(*test_letter, 'a');
                assert_eq!(*requirement_letter, 'a');
                assert_eq!(*test_position, node::Position::First);
                assert_eq!(*requirement_position, node::Position::Last);
            }
            other => panic!("expected PositionalSplit (first/last mirror) root, got {other:?}"),
        }
    }

    #[test]
    fn test_position_collision_detection() {
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
    fn test_position_to_absolute_index() {
        use node::Position;

        // Test 3-letter word (e.g., "Leo")
        assert_eq!(Position::First.to_absolute_index(3), Some(0));
        assert_eq!(Position::Second.to_absolute_index(3), Some(1));
        assert_eq!(Position::Third.to_absolute_index(3), Some(2));
        assert_eq!(Position::Last.to_absolute_index(3), Some(2));
        assert_eq!(Position::SecondToLast.to_absolute_index(3), Some(1));
        assert_eq!(Position::ThirdToLast.to_absolute_index(3), Some(0));

        // Verify Second and SecondToLast map to same index for 3-letter words
        assert_eq!(
            Position::Second.to_absolute_index(3),
            Position::SecondToLast.to_absolute_index(3),
            "Second and SecondToLast should map to same index (1) for 3-letter words"
        );

        // Test 5-letter word
        assert_eq!(Position::Third.to_absolute_index(5), Some(2));
        assert_eq!(Position::ThirdToLast.to_absolute_index(5), Some(2));

        // Contains/Double/Triple are not positional
        assert_eq!(Position::Contains.to_absolute_index(5), None);
        assert_eq!(Position::Double.to_absolute_index(5), None);
        assert_eq!(Position::Triple.to_absolute_index(5), None);
    }

    #[test]
    fn test_same_index_restriction_leo_gemini() {
        // Test with Leo and Gemini to verify the same-index restriction
        // Leo: 3 letters (l-e-o), E is at positions Second (index 1) and SecondToLast (index 1)
        // Gemini: 6 letters (g-e-m-i-n-i), E is at Second (index 1) but SecondToLast is 'n' (index 4)
        // The solver should NOT be able to chain "Second E?" with "SecondToLast E?" because
        // they refer to the same index in Leo.
        let data = words(&["leo", "gemini"]);
        let sol = minimal_trees(&data, false, true);

        // Check all generated trees to ensure none use the forbidden pattern
        for (i, tree) in sol.trees.iter().enumerate() {
            let has_forbidden_pattern = check_tree_for_forbidden_pattern(tree);
            assert!(!has_forbidden_pattern,
                "Tree {} should not contain 'Second E' followed by 'SecondToLast E' pattern:\n{}",
                i + 1, format_tree(tree));
        }
    }

    #[test]
    fn test_same_index_restriction_zodiac_subset() {
        // Test with a subset of zodiac words (those without 'R')
        // to match the context where we saw the pattern in the full zodiac output
        let data = words(&["leo", "gemini", "pisces"]);
        let sol = minimal_trees(&data, false, true);

        // Check all generated trees to ensure none use the forbidden pattern
        for (i, tree) in sol.trees.iter().enumerate() {
            let has_forbidden_pattern = check_tree_for_forbidden_pattern(tree);
            if has_forbidden_pattern {
                println!("Tree {} has forbidden pattern:\n{}", i + 1, format_tree(tree));
            }
            assert!(!has_forbidden_pattern,
                "Tree {} should not contain same-letter collision pattern", i + 1);
        }
    }

    // Helper function to check for the forbidden pattern in a tree
    fn check_tree_for_forbidden_pattern(node: &Node) -> bool {
        check_tree_for_forbidden_pattern_recursive(node, None, None)
    }

    fn check_tree_for_forbidden_pattern_recursive(
        node: &Node,
        parent_test_pos: Option<node::Position>,
        parent_test_letter: Option<char>,
    ) -> bool {
        match node {
            Node::Leaf(_) => false,
            Node::Repeat { no, .. } => {
                check_tree_for_forbidden_pattern_recursive(no, parent_test_pos, parent_test_letter)
            }
            Node::PositionalSplit {
                test_letter,
                test_position,
                yes,
                no,
                ..
            } => {
                // Check if this split violates the restriction
                if let (Some(parent_pos), Some(parent_letter)) = (parent_test_pos, parent_test_letter) {
                    if *test_letter == parent_letter {
                        use constraints::positions_can_collide;
                        if positions_can_collide(parent_pos, *test_position) {
                            return true; // Forbidden pattern found!
                        }
                    }
                }

                // Recursively check children
                check_tree_for_forbidden_pattern_recursive(yes, Some(*test_position), Some(*test_letter))
                    || check_tree_for_forbidden_pattern_recursive(no, Some(*test_position), Some(*test_letter))
            }
        }
    }

}
