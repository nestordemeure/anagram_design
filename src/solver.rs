use std::cmp::Ordering;
use std::rc::Rc;
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::cost::{compare_costs, Cost};
use crate::node::{Node, NodeRef, Solution, combine_children, combine_soft_children,
                  combine_first_letter_children, combine_soft_first_letter_children,
                  combine_last_letter_children, combine_soft_last_letter_children,
                  combine_soft_mirror_pos_children, combine_soft_double_letter_children};
use crate::constraints::{Constraints, SOFT_NO_PAIRS, split_allowed, branch_constraints};
use crate::context::{Context, mask_count, single_word_from_mask, partitions,
                     letters_present, position_mask};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    mask: u16,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    forbidden_primary: u32,
    forbidden_secondary: u32,
    allowed_primary_once: u32,
}

fn push_limited(target: &mut SmallVec<[NodeRef; 5]>, limit: Option<usize>, node: NodeRef) -> bool {
    match limit {
        Some(max) if target.len() >= max => false,
        _ => {
            target.push(node);
            true
        }
    }
}

pub(crate) fn solve(
    mask: u16,
    ctx: &Context<'_>,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    constraints: Constraints,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let present_letters = letters_present(mask, ctx);
    let constraints = constraints.prune(present_letters);

    let key = Key {
        mask,
        allow_repeat,
        prioritize_soft_no,
        forbidden_primary: constraints.forbidden_primary,
        forbidden_secondary: constraints.forbidden_secondary,
        allowed_primary_once: constraints.allowed_primary_once,
    };
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
                depth: 0,
                word_count: 1,
            },
            trees: vec![Rc::new(Node::Leaf(word))],
            exhausted: false,
        };
        memo.insert(key, sol.clone());
        return sol;
    }

    let mut best_cost: Option<Cost> = None;
    let mut best_trees: SmallVec<[NodeRef; 5]> = SmallVec::new();
    let mut exhausted = false;

    // Repeat node option: directly guess a specific word; Yes resolves that word, No continues.
    if allow_repeat && count >= 2 {
        for (idx, word) in ctx
            .words
            .iter()
            .enumerate()
            .filter(|(idx, _)| mask & (1u16 << idx) != 0)
        {
            let no_mask = mask & !(1u16 << idx);
            // Repeat can only be used once along a path: after guessing this word,
            // the remaining subtree must proceed without further repeats.
            let no_sol = solve(
                no_mask,
                ctx,
                false, // disable repeat for descendants
                prioritize_soft_no,
                constraints,
                limit,
                memo,
            );

            let yes_cost = Cost {
                nos: 0,
                hard_nos: 0,
                sum_nos: 0,
                sum_hard_nos: 0,
                depth: 0,
                word_count: 1,
            };

            let branch_cost = Cost {
                nos: no_sol.cost.nos.max(yes_cost.nos),
                hard_nos: no_sol.cost.hard_nos.max(yes_cost.hard_nos),
                sum_nos: yes_cost.sum_nos + no_sol.cost.sum_nos,
                sum_hard_nos: yes_cost.sum_hard_nos + no_sol.cost.sum_hard_nos,
                depth: std::cmp::max(yes_cost.depth, no_sol.cost.depth) + 1,
                word_count: yes_cost.word_count + no_sol.cost.word_count,
            };

            match best_cost {
                None => {
                    best_cost = Some(branch_cost);
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            Rc::new(Node::Repeat {
                                word: word.clone(),
                                no: Rc::clone(n),
                            }),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    exhausted = exhausted || no_sol.exhausted;
                }
                Some(ref current) => match compare_costs(&branch_cost, current, prioritize_soft_no) {
                    Ordering::Less => {
                        best_trees.clear();
                        best_cost = Some(branch_cost);
                        exhausted = false;
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                Rc::new(Node::Repeat {
                                    word: word.clone(),
                                    no: Rc::clone(n),
                                }),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        exhausted = exhausted || no_sol.exhausted;
                    }
                    Ordering::Equal => {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                Rc::new(Node::Repeat {
                                    word: word.clone(),
                                    no: Rc::clone(n),
                                }),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        exhausted = exhausted || no_sol.exhausted;
                    }
                    Ordering::Greater => {}
                },
            }
        }
    }

    for (idx, yes, no) in partitions(mask, &ctx.letter_masks) {
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        let letter_bit = 1u32 << idx;
        // Hard contain split; allow primary letter reuse in the YES child only.
        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            idx,
            idx,
            Some(letter_bit),
            None,
        );

        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Adding this split increases depth on both sides; the No branch increments both hard_nos and nos.
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos + 1,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1; // true tree height
                                                                                     // Calculate weighted sums: words in no branch encounter 1 additional hard no edge
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos =
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_children((b'a' + idx as u8) as char, y, n),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_children((b'a' + idx as u8) as char, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_children((b'a' + idx as u8) as char, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    // Soft split options from SOFT_NO_PAIRS
    for pair in SOFT_NO_PAIRS {
        let test_idx = (pair.test_letter as u8 - b'a') as usize;
        let requirement_idx = (pair.requirement_letter as u8 - b'a') as usize;

        let test_bit = 1u32 << test_idx;
        let requirement_bit = 1u32 << requirement_idx;

        if !split_allowed(&constraints, test_idx, requirement_idx) {
            continue;
        }

        let yes = mask & ctx.letter_masks[test_idx];
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !ctx.letter_masks[test_idx];

        // Check if all items in the "no" set contain the requirement letter
        if no & ctx.letter_masks[requirement_idx] != no {
            continue; // not all No items contain the requirement letter
        }

        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            test_idx,
            requirement_idx,
            Some(test_bit),  // soft contain can reuse P in YES
            Some(requirement_bit), // soft contain can reuse S in NO
        );
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Soft split: No branch increments nos but leaves hard_nos unchanged.
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos, // soft no does not increment hard_nos
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        // Calculate weighted sums: words in no branch encounter 1 additional soft no edge
        // (increments sum_nos but not sum_hard_nos)
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos; // no increment for soft no
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_soft_children(pair.test_letter, pair.requirement_letter, y, n),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_children(
                                    pair.test_letter,
                                    pair.requirement_letter,
                                    y,
                                    n,
                                ),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_children(
                                    pair.test_letter,
                                    pair.requirement_letter,
                                    y,
                                    n,
                                ),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    // Soft double-letter splits: Yes has two of test_letter; No has two of a different uniform letter
    for (test_idx, yes, no) in partitions(mask, &ctx.double_letter_masks) {
        if !constraints.primary_allowed(test_idx) {
            continue;
        }

        // Determine if all "no" words share a different double letter
        let mut requirement_idx_opt: Option<usize> = None;
        for idx in 0..26 {
            if idx == test_idx {
                continue;
            }
            let candidate = ctx.double_letter_masks[idx];
            if candidate & no == no {
                requirement_idx_opt = Some(idx);
                break;
            }
        }
        let requirement_idx = match requirement_idx_opt {
            Some(i) => i,
            None => continue, // no uniform double letter in no-branch
        };

        if !split_allowed(&constraints, test_idx, requirement_idx) {
            continue;
        }

        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            test_idx,
            requirement_idx,
            None,
            None,
        );

        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Soft edge: increment nos, not hard_nos
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        let test_letter = (b'a' + test_idx as u8) as char;
        let requirement_letter = (b'a' + requirement_idx as u8) as char;
        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_soft_double_letter_children(
                                test_letter,
                                requirement_letter,
                                y,
                                n,
                            ),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_double_letter_children(
                                    test_letter,
                                    requirement_letter,
                                    y,
                                    n,
                                ),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_double_letter_children(
                                    test_letter,
                                    requirement_letter,
                                    y,
                                    n,
                                ),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    // First-letter hard splits
    for (idx, yes, no) in partitions(mask, &ctx.first_letter_masks) {
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        let (yes_constraints, no_constraints) = branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Hard split on first letter: same cost structure as regular hard split
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos + 1,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos =
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_first_letter_children((b'a' + idx as u8) as char, y, n),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_first_letter_children((b'a' + idx as u8) as char, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_first_letter_children((b'a' + idx as u8) as char, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    // Soft first-letter splits: test first letter, require all No items have the same letter as second letter
    for (idx, yes, no) in partitions(mask, &ctx.first_letter_masks) {
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        // Check if all items in the "no" set have the same letter as second letter
        if no & ctx.second_letter_masks[idx] != no {
            continue;
        }

        let (yes_constraints, no_constraints) =
            branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Soft split: nos increments, but hard_nos does not
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        let letter = (b'a' + idx as u8) as char;
        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_soft_first_letter_children(letter, letter, y, n),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_first_letter_children(letter, letter, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_first_letter_children(letter, letter, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    // Positional mirror soft splits: test position from the start, require the mirror position from the end (1st↔last, 2nd↔second-to-last, 3rd↔third-to-last)
    for pos in 1..=3 {
        for idx in 0..26 {
            if !split_allowed(&constraints, idx, idx) {
                continue;
            }
            let yes = mask & position_mask(ctx, false, pos, idx);
            if yes == 0 || yes == mask {
                continue;
            }
            let no = mask & !position_mask(ctx, false, pos, idx);

            // All No items must carry the same letter in the mirrored-from-end position
            if no & position_mask(ctx, true, pos, idx) != no {
                continue;
            }

            let (yes_constraints, no_constraints) =
                branch_constraints(&constraints, idx, idx, None, None);
            let yes_sol = solve(
                yes,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                yes_constraints,
                limit,
                memo,
            );
            let no_sol = solve(
                no,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                no_constraints,
                limit,
                memo,
            );

            let yes_cost = yes_sol.cost;
            let no_cost = Cost {
                nos: no_sol.cost.nos + 1,
                hard_nos: no_sol.cost.hard_nos,
                sum_nos: no_sol.cost.sum_nos,
                sum_hard_nos: no_sol.cost.sum_hard_nos,
                depth: no_sol.cost.depth,
                word_count: no_sol.cost.word_count,
            };
            let nos = yes_cost.nos.max(no_cost.nos);
            let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
            let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
            let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
            let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
            let branch_cost = Cost {
                nos,
                hard_nos,
                sum_nos: total_sum_nos,
                sum_hard_nos: total_sum_hard_nos,
                depth: branch_depth,
                word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
            };

            let letter = (b'a' + idx as u8) as char;
            match best_cost {
                None => {
                    best_cost = Some(branch_cost);
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_mirror_pos_children(
                                    letter, pos, false, pos, true, y, n,
                                ),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                    Ordering::Less => {
                        best_trees.clear();
                        best_cost = Some(branch_cost);
                        exhausted = false;
                        for y in &yes_sol.trees {
                            for n in &no_sol.trees {
                                if !push_limited(
                                    &mut best_trees,
                                    limit,
                                    combine_soft_mirror_pos_children(
                                        letter, pos, false, pos, true, y, n,
                                    ),
                                ) {
                                    exhausted = true;
                                    break;
                                }
                            }
                            if exhausted {
                                break;
                            }
                        }
                        exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                    }
                    Ordering::Equal => {
                        for y in &yes_sol.trees {
                            for n in &no_sol.trees {
                                if !push_limited(
                                    &mut best_trees,
                                    limit,
                                    combine_soft_mirror_pos_children(
                                        letter, pos, false, pos, true, y, n,
                                    ),
                                ) {
                                    exhausted = true;
                                    break;
                                }
                            }
                            if exhausted {
                                break;
                            }
                        }
                        exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                    }
                    Ordering::Greater => {}
                },
            }
        }
    }

    // Positional mirror soft splits: test position from the end, require the mirror position from the start (last↔first, etc.)
    for pos in 1..=3 {
        for idx in 0..26 {
            if !split_allowed(&constraints, idx, idx) {
                continue;
            }
            let yes = mask & position_mask(ctx, true, pos, idx);
            if yes == 0 || yes == mask {
                continue;
            }
            let no = mask & !position_mask(ctx, true, pos, idx);

            // All No items must carry the same letter in the mirrored-from-start position
            if no & position_mask(ctx, false, pos, idx) != no {
                continue;
            }

            let (yes_constraints, no_constraints) =
                branch_constraints(&constraints, idx, idx, None, None);
            let yes_sol = solve(
                yes,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                yes_constraints,
                limit,
                memo,
            );
            let no_sol = solve(
                no,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                no_constraints,
                limit,
                memo,
            );

            let yes_cost = yes_sol.cost;
            let no_cost = Cost {
                nos: no_sol.cost.nos + 1,
                hard_nos: no_sol.cost.hard_nos,
                sum_nos: no_sol.cost.sum_nos,
                sum_hard_nos: no_sol.cost.sum_hard_nos,
                depth: no_sol.cost.depth,
                word_count: no_sol.cost.word_count,
            };
            let nos = yes_cost.nos.max(no_cost.nos);
            let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
            let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
            let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
            let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
            let branch_cost = Cost {
                nos,
                hard_nos,
                sum_nos: total_sum_nos,
                sum_hard_nos: total_sum_hard_nos,
                depth: branch_depth,
                word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
            };

            let letter = (b'a' + idx as u8) as char;
            match best_cost {
                None => {
                    best_cost = Some(branch_cost);
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_mirror_pos_children(
                                    letter, pos, true, pos, false, y, n,
                                ),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                    Ordering::Less => {
                        best_trees.clear();
                        best_cost = Some(branch_cost);
                        exhausted = false;
                        for y in &yes_sol.trees {
                            for n in &no_sol.trees {
                                if !push_limited(
                                    &mut best_trees,
                                    limit,
                                    combine_soft_mirror_pos_children(
                                        letter, pos, true, pos, false, y, n,
                                    ),
                                ) {
                                    exhausted = true;
                                    break;
                                }
                            }
                            if exhausted {
                                break;
                            }
                        }
                        exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                    }
                    Ordering::Equal => {
                        for y in &yes_sol.trees {
                            for n in &no_sol.trees {
                                if !push_limited(
                                    &mut best_trees,
                                    limit,
                                    combine_soft_mirror_pos_children(
                                        letter, pos, true, pos, false, y, n,
                                    ),
                                ) {
                                    exhausted = true;
                                    break;
                                }
                            }
                            if exhausted {
                                break;
                            }
                        }
                        exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                    }
                    Ordering::Greater => {}
                },
            }
        }
    }

    // Last-letter hard splits
    for (idx, yes, no) in partitions(mask, &ctx.last_letter_masks) {
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        let (yes_constraints, no_constraints) = branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Hard split on last letter: same cost structure as regular hard split
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos + 1,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos =
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_last_letter_children((b'a' + idx as u8) as char, y, n),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_last_letter_children((b'a' + idx as u8) as char, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_last_letter_children((b'a' + idx as u8) as char, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    // Soft last-letter splits: test last letter, require all No items have the same letter as second-to-last letter
    for (idx, yes, no) in partitions(mask, &ctx.last_letter_masks) {
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        // Check if all items in the "no" set have the same letter as second-to-last letter
        if no & ctx.second_to_last_letter_masks[idx] != no {
            continue;
        }

        let (yes_constraints, no_constraints) =
            branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

        // Soft split: nos increments, but hard_nos does not
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let nos = yes_cost.nos.max(no_cost.nos);
        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
        let branch_cost = Cost {
            nos,
            hard_nos,
            sum_nos: total_sum_nos,
            sum_hard_nos: total_sum_hard_nos,
            depth: branch_depth,
            word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
        };

        let letter = (b'a' + idx as u8) as char;
        match best_cost {
            None => {
                best_cost = Some(branch_cost);
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            &mut best_trees,
                            limit,
                            combine_soft_last_letter_children(letter, letter, y, n),
                        ) {
                            exhausted = true;
                            break;
                        }
                    }
                    if exhausted {
                        break;
                    }
                }
                exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
            }
            Some(current) => match compare_costs(&branch_cost, &current, prioritize_soft_no) {
                Ordering::Less => {
                    best_trees.clear();
                    best_cost = Some(branch_cost);
                    exhausted = false;
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_last_letter_children(letter, letter, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Equal => {
                    for y in &yes_sol.trees {
                        for n in &no_sol.trees {
                            if !push_limited(
                                &mut best_trees,
                                limit,
                                combine_soft_last_letter_children(letter, letter, y, n),
                            ) {
                                exhausted = true;
                                break;
                            }
                        }
                        if exhausted {
                            break;
                        }
                    }
                    exhausted = exhausted || yes_sol.exhausted || no_sol.exhausted;
                }
                Ordering::Greater => {}
            },
        }
    }

    let sol = Solution {
        cost: best_cost.expect("At least one tree must be found"),
        trees: best_trees.into_vec(),
        exhausted,
    };
    memo.insert(key, sol.clone());
    sol
}
