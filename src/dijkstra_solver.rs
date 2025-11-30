use std::cmp::Ordering;
use std::rc::Rc;
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::cost::{compare_costs, Cost};
use crate::node::{Node, NodeRef, Solution, Position, combine_positional_split};
use crate::constraints::{Constraints, get_reciprocal, split_allowed, branch_constraints};
use crate::context::{Context, Mask, mask_count, single_word_from_mask, partitions,
                     letters_present};

/// Memoization key for solve().
///
/// Note: prioritize_soft_no is NOT included because it's constant throughout a single
/// solve() call tree (memo is created fresh in minimal_trees and passed down).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    mask: Mask,
    forbidden: u32,
    allowed_primary_once: u32,
    allow_repeat: bool,
    parent_position: Option<Position>,
    parent_letter: Option<usize>,
}


const fn get_position_masks<'a>(ctx: &'a Context<'a>, position: Position) -> &'a [Mask; 26] {
    match position {
        Position::Contains => &ctx.letter_masks,
        Position::First => &ctx.first_letter_masks,
        Position::Second => &ctx.second_letter_masks,
        Position::Third => &ctx.third_letter_masks,
        Position::ThirdToLast => &ctx.third_to_last_letter_masks,
        Position::SecondToLast => &ctx.second_to_last_letter_masks,
        Position::Last => &ctx.last_letter_masks,
        Position::Double => &ctx.double_letter_masks,
        Position::Triple => &ctx.triple_letter_masks,
    }
}

/// Estimate lower bound cost for a state (used for candidate ordering)
/// This provides an optimistic (lower) bound that guarantees we won't prune optimal solutions
fn estimate_cost(mask: Mask, _constraints: &Constraints, _ctx: &Context<'_>, allow_repeat: bool) -> Cost {
    let count = mask_count(mask);

    if count == 1 {
        return Cost {
            nos: 0,
            hard_nos: 0,
            sum_nos: 0,
            sum_hard_nos: 0,
            word_count: 1,
        };
    }

    // Lower bounds:
    // - nos: 1 if N >= threshold, else 0
    //   - When allow_repeat=true: threshold is 3 (2 words can be handled with Repeat, nos=0)
    //   - When allow_repeat=false: threshold is 2 (need at least one split)
    // - hard_nos: 0 (optimistic: assume all soft splits)
    // - sum_nos: N-1 (balanced tree has N-1 internal nodes, each adds ≥1)
    // - sum_hard_nos: 0 (optimistic: assume all soft)
    let threshold = if allow_repeat { 3 } else { 2 };

    Cost {
        nos: if count >= threshold { 1 } else { 0 },  // Depends on allow_repeat
        hard_nos: 0,                                   // Optimistic: all soft
        sum_nos: count.saturating_sub(1),              // N-1 (balanced tree internal nodes)
        sum_hard_nos: 0,                               // Optimistic: all soft
        word_count: count,
    }
}

/// Split specification for reuse
struct SplitSpec {
    test_idx: usize,
    req_idx: usize,
    test_letter: char,
    test_position: Position,
    req_letter: char,
    req_position: Position,
    is_hard: bool,
    yes: Mask,
    no: Mask,
}

/// Generate all valid splits for a given position
fn generate_position_splits(
    position: Position,
    mask: Mask,
    ctx: &Context<'_>,
    constraints: &Constraints,
) -> Vec<SplitSpec> {
    let mut splits = Vec::new();
    let position_masks = get_position_masks(ctx, position);

    for (idx, yes, no) in partitions(mask, position_masks, &ctx.global_letters) {
        let test_letter = (b'a' + idx as u8) as char;

        // 1. Soft split with reciprocal at same position
        if let Some(reciprocal_idx) = get_reciprocal(idx) {
            if split_allowed(constraints, idx, reciprocal_idx, position) {
                let reciprocal_letter = (b'a' + reciprocal_idx as u8) as char;
                let reciprocal_masks = get_position_masks(ctx, position);
                if no & reciprocal_masks[reciprocal_idx] == no {
                    splits.push(SplitSpec {
                        test_idx: idx,
                        req_idx: reciprocal_idx,
                        test_letter,
                        test_position: position,
                        req_letter: reciprocal_letter,
                        req_position: position,
                        is_hard: false,
                        yes,
                        no,
                    });
                }
            }
        }

        // 2. Soft splits with same letter at adjacent/mirror positions
        let soft_requirement_positions: Vec<Position> = match position {
            Position::Contains => vec![],
            Position::First => vec![Position::Second, Position::Last],
            Position::Second => vec![Position::First, Position::Third, Position::SecondToLast],
            Position::Third => vec![Position::Second, Position::ThirdToLast],
            Position::ThirdToLast => vec![Position::Third, Position::SecondToLast],
            Position::SecondToLast => vec![Position::Second, Position::ThirdToLast, Position::Last],
            Position::Last => vec![Position::First, Position::SecondToLast],
            Position::Double | Position::Triple => vec![],
        };

        for req_position in soft_requirement_positions {
            // Skip if test position and requirement position can collide for any word in the NO branch
            // This prevents splits like "Second E? (No have E Second-to-last)" when No-branch words
            // have positions that refer to the same index
            // We only check the No branch because that's where the requirement applies
            let positions_collide_for_no_branch = {
                let mut collides = false;
                for (word_idx, word) in ctx.words.iter().enumerate() {
                    if no & (1 << word_idx) != 0 {
                        let word_len = word.chars().count();
                        if let (Some(idx1), Some(idx2)) = (position.to_absolute_index(word_len), req_position.to_absolute_index(word_len)) {
                            if idx1 == idx2 {
                                collides = true;
                                break;
                            }
                        }
                    }
                }
                collides
            };

            if positions_collide_for_no_branch {
                continue;
            }

            // For soft splits with same letter at different positions, check that the
            // requirement position isn't forbidden due to parent usage
            // Example: if parent used "Second E?", child can't use "... (all No have E second)"
            if let (Some(parent_pos), Some(parent_letter)) = (constraints.parent_position, constraints.parent_letter) {
                if parent_letter == idx && parent_pos == req_position {
                    // Requirement position matches parent's test position with same letter - forbidden!
                    continue;
                }
            }

            if split_allowed(constraints, idx, idx, position) {
                let req_masks = get_position_masks(ctx, req_position);
                if no & req_masks[idx] == no {
                    splits.push(SplitSpec {
                        test_idx: idx,
                        req_idx: idx,
                        test_letter,
                        test_position: position,
                        req_letter: test_letter,
                        req_position,
                        is_hard: false,
                        yes,
                        no,
                    });
                }
            }
        }

        // 3. Special handling for Double and Triple
        if matches!(position, Position::Double | Position::Triple) {
            let req_masks = get_position_masks(ctx, position);
            #[allow(clippy::needless_range_loop)]
            for req_idx in 0..26 {
                if req_idx == idx {
                    continue;
                }
                if no & req_masks[req_idx] == no && split_allowed(constraints, idx, req_idx, position) {
                    let req_letter = (b'a' + req_idx as u8) as char;
                    splits.push(SplitSpec {
                        test_idx: idx,
                        req_idx,
                        test_letter,
                        test_position: position,
                        req_letter,
                        req_position: position,
                        is_hard: false,
                        yes,
                        no,
                    });
                    break;
                }
            }
        }

        // 4. Hard split
        if split_allowed(constraints, idx, idx, position) {
            splits.push(SplitSpec {
                test_idx: idx,
                req_idx: idx,
                test_letter,
                test_position: position,
                req_letter: test_letter,
                req_position: position,
                is_hard: true,
                yes,
                no,
            });
        }
    }

    splits
}

const fn make_key(mask: Mask, constraints: &Constraints, allow_repeat: bool) -> Key {
    Key {
        mask,
        forbidden: constraints.forbidden_primary | constraints.forbidden_secondary,
        allowed_primary_once: constraints.allowed_primary_once,
        allow_repeat,
        parent_position: constraints.parent_position,
        parent_letter: constraints.parent_letter,
    }
}

pub(crate) fn solve(
    mask: Mask,
    ctx: &Context<'_>,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    constraints: Constraints,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let present_letters = letters_present(mask, ctx);
    let constraints = constraints.prune(present_letters);

    let key = make_key(mask, &constraints, allow_repeat);
    if let Some(hit) = memo.get(&key) {
        return hit.clone();
    }

    let count = mask_count(mask);

    // Leaf node
    if count == 1 {
        let word = single_word_from_mask(mask, ctx.words).expect("mask must map to a word");
        let sol = Solution {
            cost: Cost {
                nos: 0,
                hard_nos: 0,
                sum_nos: 0,
                sum_hard_nos: 0,
                word_count: 1,
            },
            trees: vec![Rc::new(Node::Leaf(word))],
        };
        memo.insert(key, sol.clone());
        return sol;
    }

    // Collect all possible split candidates with their costs
    let mut candidates: Vec<(Cost, SplitSpec)> = Vec::new();

    // Generate all possible splits across all position types
    for position in &[
        Position::Contains,
        Position::First,
        Position::Second,
        Position::Third,
        Position::ThirdToLast,
        Position::SecondToLast,
        Position::Last,
        Position::Double,
        Position::Triple,
    ] {
        let splits = generate_position_splits(*position, mask, ctx, &constraints);

        for spec in splits {
            // Estimate the cost of this split
            let est_yes = estimate_cost(spec.yes, &constraints, ctx, allow_repeat);
            let est_no = estimate_cost(spec.no, &constraints, ctx, allow_repeat);

            let est_cost = Cost {
                nos: est_yes.nos.max(est_no.nos + 1),
                hard_nos: if spec.is_hard {
                    est_yes.hard_nos.max(est_no.hard_nos + 1)
                } else {
                    est_yes.hard_nos.max(est_no.hard_nos)
                },
                sum_nos: est_yes.sum_nos + est_no.sum_nos + est_no.word_count,
                sum_hard_nos: if spec.is_hard {
                    est_yes.sum_hard_nos + est_no.sum_hard_nos + est_no.word_count
                } else {
                    est_yes.sum_hard_nos + est_no.sum_hard_nos
                },
                word_count: est_yes.word_count + est_no.word_count,
            };

            candidates.push((est_cost, spec));
        }
    }

    // Sort candidates by estimated cost (best first)
    candidates.sort_by(|a, b| compare_costs(&a.0, &b.0, prioritize_soft_no));

    let mut best_cost: Option<Cost> = None;
    let mut best_trees: SmallVec<[NodeRef; 5]> = SmallVec::new();

    // Try Repeat nodes first (if allowed)
    if allow_repeat && count >= 2 {
        for (idx, word) in ctx
            .words
            .iter()
            .enumerate()
            .filter(|(idx, _)| mask & ((1 as Mask) << idx) != 0)
        {
            let no_mask = mask & !((1 as Mask) << idx);
            // Repeat nodes don't test letters, so they break constraint chains.
            // Clear parent_position and parent_letter to prevent chaining through Repeat.
            let mut repeat_constraints = constraints.next_level();
            repeat_constraints.parent_position = None;
            repeat_constraints.parent_letter = None;
            let no_sol = solve(
                no_mask,
                ctx,
                false,
                prioritize_soft_no,
                repeat_constraints,
                memo,
            );

            if no_sol.is_unsolvable() {
                continue;
            }

            let yes_cost = Cost {
                nos: 0,
                hard_nos: 0,
                sum_nos: 0,
                sum_hard_nos: 0,
                word_count: 1,
            };

            let branch_cost = Cost {
                nos: no_sol.cost.nos.max(yes_cost.nos),
                hard_nos: no_sol.cost.hard_nos.max(yes_cost.hard_nos),
                sum_nos: yes_cost.sum_nos + no_sol.cost.sum_nos,
                sum_hard_nos: yes_cost.sum_hard_nos + no_sol.cost.sum_hard_nos,
                word_count: yes_cost.word_count + no_sol.cost.word_count,
            };

            match best_cost {
                None => {
                    best_cost = Some(branch_cost);
                    for n in &no_sol.trees {
                        best_trees.push(Rc::new(Node::Repeat {
                            word: word.clone(),
                            no: Rc::clone(n),
                        }));
                    }
                }
                Some(ref current) => match compare_costs(&branch_cost, current, prioritize_soft_no) {
                    Ordering::Less => {
                        best_trees.clear();
                        best_cost = Some(branch_cost);
                        for n in &no_sol.trees {
                            best_trees.push(Rc::new(Node::Repeat {
                                word: word.clone(),
                                no: Rc::clone(n),
                            }));
                        }
                    }
                    Ordering::Equal => {
                        for n in &no_sol.trees {
                            best_trees.push(Rc::new(Node::Repeat {
                                word: word.clone(),
                                no: Rc::clone(n),
                            }));
                        }
                    }
                    Ordering::Greater => {}
                },
            }
        }
    }

    // Process split candidates in order of estimated cost
    for (est_cost, spec) in candidates {
        // Pruning: if we already have a solution and this candidate's estimate is worse, skip
        if let Some(ref current_best) = best_cost {
            if compare_costs(&est_cost, current_best, prioritize_soft_no) == Ordering::Greater {
                continue;
            }
        }

        let test_bit = 1u32 << spec.test_idx;
        let req_bit = 1u32 << spec.req_idx;

        let (yes_allow, no_allow) = if spec.is_hard || spec.test_idx == spec.req_idx {
            (Some(test_bit), None)
        } else {
            (Some(test_bit), Some(req_bit))
        };

        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            spec.test_idx,
            spec.req_idx,
            spec.test_position,
            yes_allow,
            no_allow,
        );

        // Solve children recursively
        let no_sol = solve(spec.no, ctx, allow_repeat, prioritize_soft_no, no_constraints, memo);

        if no_sol.is_unsolvable() {
            continue;
        }

        // Pruning: check if no branch cost already exceeds best
        if let Some(ref current_best) = best_cost {
            let no_cost_nos = no_sol.cost.nos + 1;
            let no_cost_hard_nos = if spec.is_hard {
                no_sol.cost.hard_nos + 1
            } else {
                no_sol.cost.hard_nos
            };

            let can_prune = if prioritize_soft_no {
                no_cost_hard_nos > current_best.hard_nos
                    || (no_cost_hard_nos == current_best.hard_nos && no_cost_nos > current_best.nos)
            } else {
                no_cost_nos > current_best.nos
                    || (no_cost_nos == current_best.nos && no_cost_hard_nos > current_best.hard_nos)
            };

            if can_prune {
                continue;
            }
        }

        let yes_sol = solve(spec.yes, ctx, allow_repeat, prioritize_soft_no, yes_constraints, memo);

        if yes_sol.is_unsolvable() {
            continue;
        }

        // Calculate combined cost
        let yes_cost = yes_sol.cost;
        let no_cost = if spec.is_hard {
            Cost {
                nos: no_sol.cost.nos + 1,
                hard_nos: no_sol.cost.hard_nos + 1,
                sum_nos: no_sol.cost.sum_nos,
                sum_hard_nos: no_sol.cost.sum_hard_nos,
                word_count: no_sol.cost.word_count,
            }
        } else {
            Cost {
                nos: no_sol.cost.nos + 1,
                hard_nos: no_sol.cost.hard_nos,
                sum_nos: no_sol.cost.sum_nos,
                sum_hard_nos: no_sol.cost.sum_hard_nos,
                word_count: no_sol.cost.word_count,
            }
        };

        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = if spec.is_hard {
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count
        } else {
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos
        };

        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        // Update best if this is better
        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        best_trees.push(combine_positional_split(
                            spec.test_letter, spec.test_position,
                            spec.req_letter, spec.req_position, y, n
                        ));
                    }
                }
            }
            Some(ref current) => match compare_costs(&branch_cost, current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            best_trees.push(combine_positional_split(
                                spec.test_letter, spec.test_position,
                                spec.req_letter, spec.req_position, y, n
                            ));
                        }
                    }
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            best_trees.push(combine_positional_split(
                                spec.test_letter, spec.test_position,
                                spec.req_letter, spec.req_position, y, n
                            ));
                        }
                    }
                }
                Ordering::Greater => {}
            },
        }
    }

    let sol = if let Some(cost) = best_cost {
        Solution {
            cost,
            trees: best_trees.into_vec(),
        }
    } else {
        Solution::unsolvable(mask_count(mask))
    };
    memo.insert(key, sol.clone());
    sol
}
