use std::cmp::Ordering;
use std::rc::Rc;
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::cost::{compare_costs, Cost};
use crate::node::{Node, NodeRef, Solution, Position, combine_positional_split};
use crate::constraints::{Constraints, get_reciprocal, split_allowed, branch_constraints};
use crate::context::{Context, mask_count, single_word_from_mask, partitions,
                     letters_present};

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

/// Get the masks for a specific position type
fn get_position_masks<'a>(ctx: &'a Context<'a>, position: Position) -> &'a [u16; 26] {
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

/// Generate adjacent/mirror soft splits for a given position
#[allow(clippy::too_many_arguments)]
fn generate_adjacent_soft_splits(
    test_position: Position,
    test_idx: usize,
    yes: u16,
    no: u16,
    mask: u16,
    test_letter: char,
    ctx: &Context<'_>,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    constraints: &Constraints,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
    best_cost: &mut Option<Cost>,
    best_trees: &mut SmallVec<[NodeRef; 5]>,
    exhausted_flag: &mut bool,
) {
    // Define adjacent and mirror positions based on the systematization
    let soft_requirement_positions: Vec<Position> = match test_position {
        Position::Contains => vec![],  // No adjacent positions for contains
        Position::First => vec![Position::Second, Position::Last],
        Position::Second => vec![Position::First, Position::Third, Position::SecondToLast],
        Position::Third => vec![Position::Second, Position::ThirdToLast],
        Position::ThirdToLast => vec![Position::Third, Position::SecondToLast],
        Position::SecondToLast => vec![Position::Second, Position::ThirdToLast, Position::Last],
        Position::Last => vec![Position::First, Position::SecondToLast],
        Position::Double | Position::Triple => vec![],  // Special handling below
    };

    for req_position in soft_requirement_positions {
        if !split_allowed(constraints, test_idx, test_idx) {
            continue;
        }

        let req_masks = get_position_masks(ctx, req_position);
        // Check if all no items have the same letter at the requirement position
        if no & req_masks[test_idx] == no {
            try_split(
                mask,
                yes,
                no,
                test_idx,
                test_idx,  // same letter
                test_letter,
                test_position,
                test_letter,
                req_position,
                false, // is_soft
                ctx,
                allow_repeat,
                prioritize_soft_no,
                constraints,
                limit,
                memo,
                best_cost,
                best_trees,
                exhausted_flag,
            );
        }
    }

    // Special handling for Double and Triple: require a different letter doubled/tripled in No branch
    if matches!(test_position, Position::Double | Position::Triple) {
        let req_masks = get_position_masks(ctx, test_position);
        // Find a different letter that all No items have doubled/tripled
        for req_idx in 0..26 {
            if req_idx == test_idx {
                continue;
            }
            if no & req_masks[req_idx] == no {
                if split_allowed(constraints, test_idx, req_idx) {
                    let req_letter = (b'a' + req_idx as u8) as char;
                    try_split(
                        mask,
                        yes,
                        no,
                        test_idx,
                        req_idx,
                        test_letter,
                        test_position,
                        req_letter,
                        test_position,  // same position type, different letter
                        false, // is_soft
                        ctx,
                        allow_repeat,
                        prioritize_soft_no,
                        constraints,
                        limit,
                        memo,
                        best_cost,
                        best_trees,
                        exhausted_flag,
                    );
                    break;  // Only need one alternative letter
                }
            }
        }
    }
}

/// Try a specific split configuration
#[allow(clippy::too_many_arguments)]
fn try_split(
    _mask: u16,
    yes: u16,
    no: u16,
    test_idx: usize,
    req_idx: usize,
    test_letter: char,
    test_position: Position,
    req_letter: char,
    req_position: Position,
    is_hard: bool,
    ctx: &Context<'_>,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    constraints: &Constraints,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
    best_cost: &mut Option<Cost>,
    best_trees: &mut SmallVec<[NodeRef; 5]>,
    exhausted_flag: &mut bool,
) {
    // Determine exception allowances
    // Allow position splits to reuse their test letter in the yes branch
    let test_bit = 1u32 << test_idx;
    let req_bit = 1u32 << req_idx;

    let (yes_allow, no_allow) = if is_hard {
        // Hard splits: allow test letter once in yes branch
        (Some(test_bit), None)
    } else if test_idx == req_idx {
        // Soft split with same letter (mirror positions): allow in yes branch
        (Some(test_bit), None)
    } else {
        // Soft split with different letters (reciprocals/doubles): allow both
        (Some(test_bit), Some(req_bit))
    };

    let (yes_constraints, no_constraints) = branch_constraints(
        constraints,
        test_idx,
        req_idx,
        yes_allow,
        no_allow,
    );

    let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, yes_constraints, limit, memo);
    let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, no_constraints, limit, memo);

    // Calculate costs based on whether this is a hard or soft split
    let yes_cost = yes_sol.cost;
    let no_cost = if is_hard {
        Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos + 1,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        }
    } else {
        Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos,  // soft no does not increment hard_nos
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        }
    };

    let nos = yes_cost.nos.max(no_cost.nos);
    let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
    let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
    let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
    let total_sum_hard_nos = if is_hard {
        yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count
    } else {
        yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos
    };

    let branch_cost = Cost {
        nos,
        hard_nos,
        sum_nos: total_sum_nos,
        sum_hard_nos: total_sum_hard_nos,
        depth: branch_depth,
        word_count: yes_sol.cost.word_count + no_sol.cost.word_count,
    };

    // Update best if this is better
    match best_cost {
        None => {
            *best_cost = Some(branch_cost);
            for y in &yes_sol.trees {
                for n in &no_sol.trees {
                    if !push_limited(
                        best_trees,
                        limit,
                        combine_positional_split(test_letter, test_position, req_letter, req_position, y, n),
                    ) {
                        *exhausted_flag = true;
                        break;
                    }
                }
                if *exhausted_flag {
                    break;
                }
            }
            *exhausted_flag = *exhausted_flag || yes_sol.exhausted || no_sol.exhausted;
        }
        Some(ref current) => match compare_costs(&branch_cost, current, prioritize_soft_no) {
            Ordering::Less => {
                best_trees.clear();
                *best_cost = Some(branch_cost);
                *exhausted_flag = false;
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            best_trees,
                            limit,
                            combine_positional_split(test_letter, test_position, req_letter, req_position, y, n),
                        ) {
                            *exhausted_flag = true;
                            break;
                        }
                    }
                    if *exhausted_flag {
                        break;
                    }
                }
                *exhausted_flag = *exhausted_flag || yes_sol.exhausted || no_sol.exhausted;
            }
            Ordering::Equal => {
                for y in &yes_sol.trees {
                    for n in &no_sol.trees {
                        if !push_limited(
                            best_trees,
                            limit,
                            combine_positional_split(test_letter, test_position, req_letter, req_position, y, n),
                        ) {
                            *exhausted_flag = true;
                            break;
                        }
                    }
                    if *exhausted_flag {
                        break;
                    }
                }
                *exhausted_flag = *exhausted_flag || yes_sol.exhausted || no_sol.exhausted;
            }
            Ordering::Greater => {}
        },
    }
}

/// Try to generate splits for a given position type with all systematized soft variants
#[allow(clippy::too_many_arguments)]
fn try_position_splits(
    position: Position,
    mask: u16,
    ctx: &Context<'_>,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    constraints: &Constraints,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
    best_cost: &mut Option<Cost>,
    best_trees: &mut SmallVec<[NodeRef; 5]>,
    exhausted_flag: &mut bool,
) {
    let position_masks = get_position_masks(ctx, position);

    // Generate all partitions for this position
    for (idx, yes, no) in partitions(mask, position_masks) {
        let test_letter = (b'a' + idx as u8) as char;

        // 1. Hard split (test == requirement)
        if split_allowed(constraints, idx, idx) {
            try_split(
                mask,
                yes,
                no,
                idx,
                idx,
                test_letter,
                position,
                test_letter,
                position,
                true, // is_hard
                ctx,
                allow_repeat,
                prioritize_soft_no,
                constraints,
                limit,
                memo,
                best_cost,
                best_trees,
                exhausted_flag,
            );
        }

        // 2. Soft split with reciprocal at same position
        if let Some(reciprocal_idx) = get_reciprocal(idx) {
            if split_allowed(constraints, idx, reciprocal_idx) {
                let reciprocal_letter = (b'a' + reciprocal_idx as u8) as char;
                // Check if all no items have the reciprocal at the same position
                let reciprocal_masks = get_position_masks(ctx, position);
                if no & reciprocal_masks[reciprocal_idx] == no {
                    try_split(
                        mask,
                        yes,
                        no,
                        idx,
                        reciprocal_idx,
                        test_letter,
                        position,
                        reciprocal_letter,
                        position,
                        false, // is_soft
                        ctx,
                        allow_repeat,
                        prioritize_soft_no,
                        constraints,
                        limit,
                        memo,
                        best_cost,
                        best_trees,
                        exhausted_flag,
                    );
                }
            }
        }

        // 3. Soft splits with same letter at adjacent/mirror positions
        generate_adjacent_soft_splits(
            position,
            idx,
            yes,
            no,
            mask,
            test_letter,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            constraints,
            limit,
            memo,
            best_cost,
            best_trees,
            exhausted_flag,
        );
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

    // Generate all systematized splits for each position type
    // Currently using 4 positions that the old code supported
    // TODO: Improve constraint system to support all 9 position types (Second, Third, ThirdToLast, SecondToLast, Triple)
    try_position_splits(Position::Contains, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    try_position_splits(Position::First, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    try_position_splits(Position::Last, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    try_position_splits(Position::Double, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    // These additional positions cause constraint exhaustion and need better exception handling:
    // try_position_splits(Position::Second, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    // try_position_splits(Position::Third, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    // try_position_splits(Position::ThirdToLast, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    // try_position_splits(Position::SecondToLast, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);
    // try_position_splits(Position::Triple, mask, ctx, allow_repeat, prioritize_soft_no, &constraints, limit, memo, &mut best_cost, &mut best_trees, &mut exhausted);

    let sol = if let Some(cost) = best_cost {
        Solution {
            cost,
            trees: best_trees.into_vec(),
            exhausted,
        }
    } else {
        panic!("At least one tree must be found");
    };
    memo.insert(key, sol.clone());
    sol
}
