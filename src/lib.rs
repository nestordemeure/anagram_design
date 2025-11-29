// Module declarations
pub mod cost;
pub mod node;
pub mod constraints;
pub mod context;
pub mod solver;
pub mod format;
pub mod api;
pub mod merged;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-export public API
pub use cost::{Cost, compare_costs};
pub use node::{Node, NodeRef, Solution};
pub use format::format_tree;
pub use api::{minimal_trees, minimal_trees_limited};
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

    /// Recompute the full Cost of a tree by walking it, independent of the solver.
    fn compute_cost(node: &Node) -> Cost {
        match node {
            Node::Leaf(_) => Cost {
                nos: 0,
                hard_nos: 0,
                sum_nos: 0,
                sum_hard_nos: 0,
                depth: 0,
                word_count: 1,
            },
            Node::Repeat { no, .. } => {
                let yes_cost = Cost {
                    nos: 0,
                    hard_nos: 0,
                    sum_nos: 0,
                    sum_hard_nos: 0,
                    depth: 0,
                    word_count: 1,
                };
                let no_cost = compute_cost(no);
                Cost {
                    nos: yes_cost.nos.max(no_cost.nos),
                    hard_nos: yes_cost.hard_nos.max(no_cost.hard_nos),
                    sum_nos: yes_cost.sum_nos + no_cost.sum_nos,
                    sum_hard_nos: yes_cost.sum_hard_nos + no_cost.sum_hard_nos,
                    depth: yes_cost.depth.max(no_cost.depth) + 1,
                    word_count: yes_cost.word_count + no_cost.word_count,
                }
            }
            Node::PositionalSplit {
                test_letter,
                test_position,
                requirement_letter,
                requirement_position,
                yes,
                no,
            } => {
                let yes_cost = compute_cost(yes);
                let no_cost_base = compute_cost(no);

                // Determine if this is a hard split
                let is_hard = test_letter == requirement_letter && test_position == requirement_position;

                let no_cost = if is_hard {
                    Cost {
                        nos: no_cost_base.nos + 1,
                        hard_nos: no_cost_base.hard_nos + 1,
                        sum_nos: no_cost_base.sum_nos,
                        sum_hard_nos: no_cost_base.sum_hard_nos,
                        depth: no_cost_base.depth,
                        word_count: no_cost_base.word_count,
                    }
                } else {
                    Cost {
                        nos: no_cost_base.nos + 1,
                        hard_nos: no_cost_base.hard_nos,
                        sum_nos: no_cost_base.sum_nos,
                        sum_hard_nos: no_cost_base.sum_hard_nos,
                        depth: no_cost_base.depth,
                        word_count: no_cost_base.word_count,
                    }
                };

                let nos = yes_cost.nos.max(no_cost.nos);
                let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
                let depth = yes_cost.depth.max(no_cost.depth) + 1;
                let sum_nos = yes_cost.sum_nos + no_cost.sum_nos + no_cost.word_count;
                let sum_hard_nos = if is_hard {
                    yes_cost.sum_hard_nos + no_cost.sum_hard_nos + no_cost.word_count
                } else {
                    yes_cost.sum_hard_nos + no_cost.sum_hard_nos
                };

                Cost {
                    nos,
                    hard_nos,
                    sum_nos,
                    sum_hard_nos,
                    depth,
                    word_count: yes_cost.word_count + no_cost.word_count,
                }
            }
        }
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
        let allow_repeat = minimal_trees_limited(&data, true, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, true, Some(1));
        // With the improved unified architecture and better exception handling,
        // we achieve better (lower) sum_nos and sum_hard_nos costs
        assert_eq!(
            allow_repeat.cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 11,
                sum_hard_nos: 6,
                depth: 6,
                word_count: 12
            }
        );
        assert_eq!(
            no_repeat.cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 16,
                sum_hard_nos: 8,
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
    fn recomputed_cost_matches_expected_for_top_tree() {
        // Use the first printed allow_repeat tree to assert its true hard_no count.
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
        let sol = minimal_trees_limited(&data, true, true, Some(1));
        let tree = &sol.trees[0];
        let cost = compute_cost(tree);
        assert_eq!(
            cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 11,
                sum_hard_nos: 6,
                depth: 6,
                word_count: 12
            }
        );
    }

    #[test]
    fn solver_advertised_cost_matches_tree_cost_allow_repeat() {
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
        let sol = minimal_trees_limited(&data, true, true, Some(3));
        for (idx, tree) in sol.trees.iter().take(3).enumerate() {
            let tree_cost = compute_cost(tree);
            assert_eq!(
                sol.cost,
                tree_cost,
                "Tree {} cost mismatch: solver reported {:?}, recomputed {:?}",
                idx + 1,
                sol.cost,
                tree_cost
            );
        }
    }

    #[test]
    fn soft_double_letter_split_works() {
        // Yes: words with double 'o'; No: words with double 'l'
        let data = words(&["book", "pool", "ball", "tall"]);
        let sol = minimal_trees_limited(&data, false, true, Some(1));
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
        match &*sol.trees[0] {
            Node::PositionalSplit {
                test_letter: 'l',
                test_position: node::Position::Contains,
                requirement_letter: 'l',
                requirement_position: node::Position::Contains,
                yes,
                no,
            } => {
                assert_eq!(leaves(no), vec!["book".to_string()]);
                if let Node::PositionalSplit {
                    test_letter,
                    test_position: node::Position::Double,
                    requirement_letter,
                    requirement_position: node::Position::Double,
                    yes: yes_branch,
                    no: no_branch,
                } = &**yes
                {
                    let pair = (*test_letter, *requirement_letter);
                    assert!(
                        pair == ('l', 'o') || pair == ('o', 'l'),
                        "expected letters l/o in some order, got {pair:?}"
                    );
                    let mut yes_leaves = leaves(yes_branch);
                    yes_leaves.sort();
                    assert_eq!(yes_leaves, vec!["ball".to_string(), "tall".to_string()]);

                    let mut no_leaves = leaves(no_branch);
                    no_leaves.sort();
                    assert_eq!(no_leaves, vec!["pool".to_string()]);
                } else {
                    panic!("expected soft double letter split after 'l' split, got {:?}", &**yes);
                }
            }
            other => panic!("expected leading 'l' hard split, got {other:?}"),
        }
    }

    #[test]
    fn soft_mirror_first_last_split_works() {
        // Front test, back requirement mirror keeps the miss soft
        let data = words(&["axe", "exa"]);
        let sol = minimal_trees_limited(&data, false, true, Some(1));
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
}
