use std::cmp::Ordering;
use std::rc::Rc;
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::cost::{add_no_edge, add_yes_split, compare_costs, estimate_cost, Cost};
use crate::node::{Node, NodeRef, Solution, Position, combine_positional_split, combine_yes_split};
use crate::constraints::{Constraints, get_reciprocal, split_allowed, branch_constraints};
use crate::context::{Context, Mask, mask_count, single_word_from_mask, partitions, letters_present};

/// Memoization key for solve().
///
/// Note: prioritize_soft_no is NOT included because it's constant throughout a single
/// solve() call tree (memo is created fresh in minimal_trees and passed down).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Key
{
    mask: Mask,
    forbidden: u32,
    allowed_primary_once: u32,
    allow_repeat: bool,
    parent_position: Option<Position>,
    parent_letter: Option<usize>
}

const fn get_position_masks<'a>(ctx: &'a Context<'a>, position: Position) -> &'a [Mask; 26]
{
    match position
    {
        Position::Contains => &ctx.letter_masks,
        Position::First => &ctx.first_letter_masks,
        Position::Second => &ctx.second_letter_masks,
        Position::Third => &ctx.third_letter_masks,
        Position::ThirdToLast => &ctx.third_to_last_letter_masks,
        Position::SecondToLast => &ctx.second_to_last_letter_masks,
        Position::Last => &ctx.last_letter_masks,
        Position::Double => &ctx.double_letter_masks,
        Position::Triple => &ctx.triple_letter_masks
    }
}

/// Split specification for reuse
struct SplitSpec
{
    test_idx: usize,
    req_idx: usize,
    test_letter: char,
    test_position: Position,
    req_letter: char,
    req_position: Position,
    is_hard: bool,
    yes: Mask,
    no: Mask
}

/// Find all valid YesSplits for a mask.
/// YesSplits are hard splits that are true for ALL words in the mask.
/// parent_hard_split: If Some((idx, pos)), the parent is a hard split and we should
/// avoid yes splits that would be equivalent to a soft split (parent_primary, yes_split_question).
fn find_valid_yes_splits(mask: Mask,
                         ctx: &Context<'_>,
                         constraints: &Constraints,
                         parent_hard_split: Option<(usize, Position)>)
                         -> Vec<(Position, usize, char)>
{
    let mut valid_yes_splits = Vec::new();

    // Try all position types
    for position in &[Position::Contains,
                      Position::First,
                      Position::Second,
                      Position::Third,
                      Position::ThirdToLast,
                      Position::SecondToLast,
                      Position::Last,
                      Position::Double,
                      Position::Triple]
    {
        let position_masks = get_position_masks(ctx, *position);

        // Check each letter
        for (idx, &letter_mask) in position_masks.iter().enumerate().take(26)
        {
            // YesSplit is valid if ALL words in mask have this property
            // (i.e., yes == mask, no == 0)
            if mask & letter_mask == mask
            {
                // Check if this split is allowed by constraints (like hard splits)
                if split_allowed(constraints, idx, idx, *position)
                {
                    // Check if this yes split would be redundant with a soft split
                    // (only for immediate children of hard splits on the no-branch)
                    if let Some((parent_idx, parent_pos)) = parent_hard_split
                    {
                        use crate::constraints::would_form_soft_split;
                        if would_form_soft_split(parent_idx, parent_pos, idx, *position)
                        {
                            // Skip this yes split - it's redundant with the soft split
                            continue;
                        }
                    }

                    let letter = (b'a' + idx as u8) as char;
                    valid_yes_splits.push((*position, idx, letter));
                }
            }
        }
    }

    valid_yes_splits
}

/// Generate all valid splits for a given position
fn generate_position_splits(position: Position,
                            mask: Mask,
                            ctx: &Context<'_>,
                            constraints: &Constraints)
                            -> Vec<SplitSpec>
{
    let mut splits = Vec::new();
    let position_masks = get_position_masks(ctx, position);

    for (idx, yes, no) in partitions(mask, position_masks, &ctx.global_letters)
    {
        let test_letter = (b'a' + idx as u8) as char;

        // 1. Soft split with reciprocal at same position
        if let Some(reciprocal_idx) = get_reciprocal(idx)
        {
            if split_allowed(constraints, idx, reciprocal_idx, position)
            {
                let reciprocal_letter = (b'a' + reciprocal_idx as u8) as char;
                let reciprocal_masks = get_position_masks(ctx, position);
                if no & reciprocal_masks[reciprocal_idx] == no
                {
                    splits.push(SplitSpec { test_idx: idx,
                                            req_idx: reciprocal_idx,
                                            test_letter,
                                            test_position: position,
                                            req_letter: reciprocal_letter,
                                            req_position: position,
                                            is_hard: false,
                                            yes,
                                            no });
                }
            }
        }

        // 2. Soft splits with same letter at adjacent/mirror positions
        let soft_requirement_positions: Vec<Position> = match position
        {
            Position::Contains => vec![],
            Position::First => vec![Position::Second, Position::Last],
            Position::Second => vec![Position::First, Position::Third, Position::SecondToLast],
            Position::Third => vec![Position::Second, Position::ThirdToLast],
            Position::ThirdToLast => vec![Position::Third, Position::SecondToLast],
            Position::SecondToLast => vec![Position::Second, Position::ThirdToLast, Position::Last],
            Position::Last => vec![Position::First, Position::SecondToLast],
            Position::Double | Position::Triple => vec![]
        };

        for req_position in soft_requirement_positions
        {
            // Skip if test position and requirement position can collide for any word in the NO branch
            // This prevents splits like "Second E? (No have E Second-to-last)" when No-branch words
            // have positions that refer to the same index
            // We only check the No branch because that's where the requirement applies
            let positions_collide_for_no_branch = {
                let mut collides = false;
                for (word_idx, word) in ctx.words.iter().enumerate()
                {
                    if no & (1 << word_idx) != 0
                    {
                        let word_len = word.chars().count();
                        if let (Some(idx1), Some(idx2)) =
                            (position.to_absolute_index(word_len), req_position.to_absolute_index(word_len))
                        {
                            if idx1 == idx2
                            {
                                collides = true;
                                break;
                            }
                        }
                    }
                }
                collides
            };

            if positions_collide_for_no_branch
            {
                continue;
            }

            // For soft splits with same letter at different positions, check that the
            // requirement position isn't forbidden due to parent usage
            // Example: if parent used "Second E?", child can't use "... (all No have E second)"
            if let (Some(parent_pos), Some(parent_letter)) =
                (constraints.parent_position, constraints.parent_letter)
            {
                if parent_letter == idx && parent_pos == req_position
                {
                    // Requirement position matches parent's test position with same letter - forbidden!
                    continue;
                }
            }

            if split_allowed(constraints, idx, idx, position)
            {
                let req_masks = get_position_masks(ctx, req_position);
                if no & req_masks[idx] == no
                {
                    splits.push(SplitSpec { test_idx: idx,
                                            req_idx: idx,
                                            test_letter,
                                            test_position: position,
                                            req_letter: test_letter,
                                            req_position,
                                            is_hard: false,
                                            yes,
                                            no });
                }
            }
        }

        // 3. Special handling for Double and Triple
        if matches!(position, Position::Double | Position::Triple)
        {
            let req_masks = get_position_masks(ctx, position);
            #[allow(clippy::needless_range_loop)]
            for req_idx in 0..26
            {
                if req_idx == idx
                {
                    continue;
                }
                if no & req_masks[req_idx] == no && split_allowed(constraints, idx, req_idx, position)
                {
                    let req_letter = (b'a' + req_idx as u8) as char;
                    splits.push(SplitSpec { test_idx: idx,
                                            req_idx,
                                            test_letter,
                                            test_position: position,
                                            req_letter,
                                            req_position: position,
                                            is_hard: false,
                                            yes,
                                            no });
                    break;
                }
            }
        }

        // 4. Hard split
        if split_allowed(constraints, idx, idx, position)
        {
            splits.push(SplitSpec { test_idx: idx,
                                    req_idx: idx,
                                    test_letter,
                                    test_position: position,
                                    req_letter: test_letter,
                                    req_position: position,
                                    is_hard: true,
                                    yes,
                                    no });
        }
    }

    splits
}

const fn make_key(mask: Mask, constraints: &Constraints, allow_repeat: bool) -> Key
{
    Key { mask,
          forbidden: constraints.forbidden_primary | constraints.forbidden_secondary,
          allowed_primary_once: constraints.allowed_primary_once,
          allow_repeat,
          parent_position: constraints.parent_position,
          parent_letter: constraints.parent_letter }
}

pub(crate) fn solve(mask: Mask,
                    ctx: &Context<'_>,
                    allow_repeat: bool,
                    prioritize_soft_no: bool,
                    redeeming_yes: u32,
                    constraints: Constraints,
                    memo: &mut HashMap<Key, Solution>)
                    -> Solution
{
    let present_letters = letters_present(mask, ctx);
    let constraints = constraints.prune(present_letters);

    let key = make_key(mask, &constraints, allow_repeat);
    if let Some(hit) = memo.get(&key)
    {
        return hit.clone();
    }

    let count = mask_count(mask);

    // Leaf node
    if count == 1
    {
        let word = single_word_from_mask(mask, ctx.words).expect("mask must map to a word");
        let sol = Solution { cost: Cost { hard_nos: 0,
                                          redeemed_hard_nos: 0,
                                          nos: 0,
                                          redeemed_nos: 0,
                                          sum_hard_nos: 0,
                                          redeemed_sum_hard_nos: 0,
                                          sum_nos: 0,
                                          redeemed_sum_nos: 0,
                                          word_count: 1 },
                             trees: vec![Rc::new(Node::Leaf(word))] };
        memo.insert(key, sol.clone());
        return sol;
    }

    // Collect all possible split candidates with their costs
    let mut candidates: Vec<(Cost, SplitSpec)> = Vec::new();

    // Generate all possible splits across all position types
    for position in &[Position::Contains,
                      Position::First,
                      Position::Second,
                      Position::Third,
                      Position::ThirdToLast,
                      Position::SecondToLast,
                      Position::Last,
                      Position::Double,
                      Position::Triple]
    {
        let splits = generate_position_splits(*position, mask, ctx, &constraints);

        for spec in splits
        {
            // Estimate the cost of this split
            let est_yes = estimate_cost(spec.yes, allow_repeat, redeeming_yes);
            let est_no = estimate_cost(spec.no, allow_repeat, redeeming_yes);

            let hard_nos = if spec.is_hard
            {
                est_yes.hard_nos.max(est_no.hard_nos + 1)
            }
            else
            {
                est_yes.hard_nos.max(est_no.hard_nos)
            };
            let redeemed_hard_nos = if spec.is_hard
            {
                est_yes.redeemed_hard_nos.max(est_no.redeemed_hard_nos + redeeming_yes as i32)
            }
            else
            {
                est_yes.redeemed_hard_nos.max(est_no.redeemed_hard_nos)
            };
            let nos = est_yes.nos.max(est_no.nos + 1);
            let redeemed_nos = est_yes.redeemed_nos.max(est_no.redeemed_nos + redeeming_yes as i32);
            let sum_hard_nos = if spec.is_hard
            {
                est_yes.sum_hard_nos + est_no.sum_hard_nos + est_no.word_count
            }
            else
            {
                est_yes.sum_hard_nos + est_no.sum_hard_nos
            };
            let redeemed_sum_hard_nos = if spec.is_hard
            {
                est_yes.redeemed_sum_hard_nos
                + est_no.redeemed_sum_hard_nos
                + (est_no.word_count as i32 * redeeming_yes as i32)
            }
            else
            {
                est_yes.redeemed_sum_hard_nos + est_no.redeemed_sum_hard_nos
            };
            let sum_nos = est_yes.sum_nos + est_no.sum_nos + est_no.word_count;
            let redeemed_sum_nos = est_yes.redeemed_sum_nos
                                   + est_no.redeemed_sum_nos
                                   + (est_no.word_count as i32 * redeeming_yes as i32);

            let est_cost = Cost { hard_nos,
                                  redeemed_hard_nos,
                                  nos,
                                  redeemed_nos,
                                  sum_hard_nos,
                                  redeemed_sum_hard_nos,
                                  sum_nos,
                                  redeemed_sum_nos,
                                  word_count: est_yes.word_count + est_no.word_count };

            candidates.push((est_cost, spec));
        }
    }

    // Sort candidates by estimated cost (best first)
    candidates.sort_by(|a, b| compare_costs(&a.0, &b.0, prioritize_soft_no));

    let mut best_cost: Option<Cost> = None;
    let mut best_trees: SmallVec<[NodeRef; 5]> = SmallVec::new();

    // Try Repeat nodes first (if allowed)
    if allow_repeat && count >= 2
    {
        for (idx, word) in ctx.words.iter().enumerate().filter(|(idx, _)| mask & ((1 as Mask) << idx) != 0)
        {
            let no_mask = mask & !((1 as Mask) << idx);
            // Repeat nodes don't test letters, so they break constraint chains.
            // Clear parent_position and parent_letter to prevent chaining through Repeat.
            let mut repeat_constraints = constraints.next_level();
            repeat_constraints.parent_position = None;
            repeat_constraints.parent_letter = None;
            let no_sol =
                solve(no_mask, ctx, false, prioritize_soft_no, redeeming_yes, repeat_constraints, memo);

            if no_sol.is_unsolvable()
            {
                continue;
            }

            let yes_cost = Cost { hard_nos: 0,
                                  redeemed_hard_nos: 0,
                                  nos: 0,
                                  redeemed_nos: 0,
                                  sum_hard_nos: 0,
                                  redeemed_sum_hard_nos: 0,
                                  sum_nos: 0,
                                  redeemed_sum_nos: 0,
                                  word_count: 1 };

            let branch_cost =
                Cost { hard_nos: no_sol.cost.hard_nos.max(yes_cost.hard_nos),
                       redeemed_hard_nos: no_sol.cost.redeemed_hard_nos.max(yes_cost.redeemed_hard_nos),
                       nos: no_sol.cost.nos.max(yes_cost.nos),
                       redeemed_nos: no_sol.cost.redeemed_nos.max(yes_cost.redeemed_nos),
                       sum_hard_nos: yes_cost.sum_hard_nos + no_sol.cost.sum_hard_nos,
                       redeemed_sum_hard_nos: yes_cost.redeemed_sum_hard_nos
                                              + no_sol.cost.redeemed_sum_hard_nos,
                       sum_nos: yes_cost.sum_nos + no_sol.cost.sum_nos,
                       redeemed_sum_nos: yes_cost.redeemed_sum_nos + no_sol.cost.redeemed_sum_nos,
                       word_count: yes_cost.word_count + no_sol.cost.word_count };

            match best_cost
            {
                None =>
                {
                    best_cost = Some(branch_cost);
                    for n in &no_sol.trees
                    {
                        best_trees.push(Rc::new(Node::Repeat { word: word.clone(), no: Rc::clone(n) }));
                    }
                }
                Some(ref current) => match compare_costs(&branch_cost, current, prioritize_soft_no)
                {
                    Ordering::Less =>
                    {
                        best_trees.clear();
                        best_cost = Some(branch_cost);
                        for n in &no_sol.trees
                        {
                            best_trees.push(Rc::new(Node::Repeat { word: word.clone(), no: Rc::clone(n) }));
                        }
                    }
                    Ordering::Equal =>
                    {
                        for n in &no_sol.trees
                        {
                            best_trees.push(Rc::new(Node::Repeat { word: word.clone(), no: Rc::clone(n) }));
                        }
                    }
                    Ordering::Greater =>
                    {}
                }
            }
        }
    }

    // Process split candidates in order of estimated cost
    for (est_cost, spec) in candidates
    {
        // Pruning: if we already have a solution and this candidate's estimate is worse, skip
        if let Some(ref current_best) = best_cost
        {
            if compare_costs(&est_cost, current_best, prioritize_soft_no) == Ordering::Greater
            {
                continue;
            }
        }

        let test_bit = 1u32 << spec.test_idx;
        let req_bit = 1u32 << spec.req_idx;

        let (yes_allow, no_allow) = if spec.is_hard || spec.test_idx == spec.req_idx
        {
            (Some(test_bit), None)
        }
        else
        {
            (Some(test_bit), Some(req_bit))
        };

        let (yes_constraints, no_constraints) = branch_constraints(&constraints,
                                                                   spec.test_idx,
                                                                   spec.req_idx,
                                                                   spec.test_position,
                                                                   yes_allow,
                                                                   no_allow);

        // Solve children recursively
        let no_sol =
            solve(spec.no, ctx, allow_repeat, prioritize_soft_no, redeeming_yes, no_constraints, memo);

        if no_sol.is_unsolvable()
        {
            continue;
        }

        // Pruning: check if no branch cost already exceeds best
        // Account for potential YesSplits that could reduce cost
        if let Some(ref current_best) = best_cost
        {
            let mut no_cost = add_no_edge(&no_sol.cost, spec.is_hard, redeeming_yes as i32);

            // Optimistically assume we can add up to redeeming_yes YesSplits
            let no_word_count = mask_count(spec.no);
            let min_words_for_yes_split = if allow_repeat { 3 } else { 2 };

            let max_yes_splits =
                if redeeming_yes > 0 && no_word_count >= min_words_for_yes_split { redeeming_yes } else { 0 };

            // Apply maximum possible YesSplits (reduces redeemed costs)
            for _ in 0..max_yes_splits
            {
                no_cost = add_yes_split(&no_cost);
            }

            // Cap redeemed costs to not be negative
            no_cost.redeemed_hard_nos = no_cost.redeemed_hard_nos.max(0);
            no_cost.redeemed_nos = no_cost.redeemed_nos.max(0);
            no_cost.redeemed_sum_hard_nos = no_cost.redeemed_sum_hard_nos.max(0);
            no_cost.redeemed_sum_nos = no_cost.redeemed_sum_nos.max(0);

            // Use compare_costs to check if even with max YesSplits, this no branch is worse than best
            if compare_costs(&no_cost, current_best, prioritize_soft_no) == Ordering::Greater
            {
                continue;
            }
        }

        let yes_sol =
            solve(spec.yes, ctx, allow_repeat, prioritize_soft_no, redeeming_yes, yes_constraints, memo);

        if yes_sol.is_unsolvable()
        {
            continue;
        }

        // Helper to process a split (base or augmented with YesSplits)
        let mut process_split = |no_branch_node: &NodeRef, yes_split_count: u32| {
            // Calculate cost with YesSplit adjustments
            let mut no_cost = add_no_edge(&no_sol.cost, spec.is_hard, redeeming_yes as i32);

            // Apply YesSplit cost adjustments (-1 per YesSplit)
            for _ in 0..yes_split_count
            {
                no_cost = add_yes_split(&no_cost);
            }

            // Add word_count contributions to no_cost sum metrics
            no_cost.sum_nos += no_sol.cost.word_count;
            no_cost.redeemed_sum_nos += no_sol.cost.word_count as i32 * redeeming_yes as i32;
            if spec.is_hard
            {
                no_cost.sum_hard_nos += no_sol.cost.word_count;
                no_cost.redeemed_sum_hard_nos += no_sol.cost.word_count as i32 * redeeming_yes as i32;
            }

            // Cap redeemed costs to not be negative
            no_cost.redeemed_hard_nos = no_cost.redeemed_hard_nos.max(0);
            no_cost.redeemed_nos = no_cost.redeemed_nos.max(0);
            no_cost.redeemed_sum_hard_nos = no_cost.redeemed_sum_hard_nos.max(0);
            no_cost.redeemed_sum_nos = no_cost.redeemed_sum_nos.max(0);

            let yes_cost = add_yes_split(&yes_sol.cost);
            let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
            let redeemed_hard_nos = yes_cost.redeemed_hard_nos.max(no_cost.redeemed_hard_nos);
            let nos = yes_cost.nos.max(no_cost.nos);
            let redeemed_nos = yes_cost.redeemed_nos.max(no_cost.redeemed_nos);
            let total_sum_nos = yes_sol.cost.sum_nos + no_cost.sum_nos;
            let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_cost.sum_hard_nos;
            let total_redeemed_sum_nos = yes_sol.cost.redeemed_sum_nos + no_cost.redeemed_sum_nos;
            let total_redeemed_sum_hard_nos =
                yes_sol.cost.redeemed_sum_hard_nos + no_cost.redeemed_sum_hard_nos;

            let branch_cost = Cost { hard_nos,
                                     redeemed_hard_nos,
                                     nos,
                                     redeemed_nos,
                                     sum_hard_nos: total_sum_hard_nos,
                                     redeemed_sum_hard_nos: total_redeemed_sum_hard_nos,
                                     sum_nos: total_sum_nos,
                                     redeemed_sum_nos: total_redeemed_sum_nos,
                                     word_count: yes_sol.cost.word_count + no_sol.cost.word_count };

            // Update best if this is better
            match best_cost
            {
                None =>
                {
                    best_cost = Some(branch_cost);
                    for y in &yes_sol.trees
                    {
                        best_trees.push(combine_positional_split(spec.test_letter,
                                                                 spec.test_position,
                                                                 spec.req_letter,
                                                                 spec.req_position,
                                                                 y,
                                                                 no_branch_node));
                    }
                }
                Some(ref current) => match compare_costs(&branch_cost, current, prioritize_soft_no)
                {
                    Ordering::Less =>
                    {
                        best_trees.clear();
                        best_cost = Some(branch_cost);
                        for y in &yes_sol.trees
                        {
                            best_trees.push(combine_positional_split(spec.test_letter,
                                                                     spec.test_position,
                                                                     spec.req_letter,
                                                                     spec.req_position,
                                                                     y,
                                                                     no_branch_node));
                        }
                    }
                    Ordering::Equal =>
                    {
                        for y in &yes_sol.trees
                        {
                            best_trees.push(combine_positional_split(spec.test_letter,
                                                                     spec.test_position,
                                                                     spec.req_letter,
                                                                     spec.req_position,
                                                                     y,
                                                                     no_branch_node));
                        }
                    }
                    Ordering::Greater =>
                    {}
                }
            }
        };

        // Process base split (no YesSplits)
        for n in &no_sol.trees
        {
            process_split(n, 0);
        }

        // Build YesSplit layers incrementally with re-solving
        // Only add YesSplits if no branch has enough words and redeeming_yes > 0
        let no_word_count = mask_count(spec.no);
        let min_words_for_yes_split = if allow_repeat { 3 } else { 2 };

        if redeeming_yes > 0 && no_word_count >= min_words_for_yes_split
        {
            // Track (position, letter_idx) pairs already used in parent split
            let parent_exclusions: SmallVec<[(Position, usize); 4]> =
                SmallVec::from_vec(vec![(spec.test_position, spec.test_idx),
                                        (spec.req_position, spec.req_idx)]);

            // If parent is a hard split, pass info to avoid redundant yes splits
            let parent_hard_split =
                if spec.is_hard { Some((spec.test_idx, spec.test_position)) } else { None };

            // Current layer: (solution, constraints, yessplit_chain)
            type LayerEntry = (Solution, Constraints, SmallVec<[(Position, usize, char); 8]>);
            let mut current_layer: Vec<LayerEntry> = vec![(no_sol.clone(), no_constraints, SmallVec::new())];

            // Build layers incrementally
            for _layer_num in 1..=redeeming_yes
            {
                let mut next_layer: Vec<LayerEntry> = Vec::new();

                for (_prev_sol, prev_constraints, prev_chain) in &current_layer
                {
                    // Find valid YesSplits at this layer (only immediate children for first layer)
                    let parent_hard_split_filter =
                        if prev_chain.is_empty() { parent_hard_split } else { None };
                    let valid_splits =
                        find_valid_yes_splits(spec.no, ctx, prev_constraints, parent_hard_split_filter);

                    for (position, idx, letter) in valid_splits
                    {
                        // Skip if already used in parent split, previous chain, or marked as failed
                        let already_used = parent_exclusions.iter().any(|(p, i)| *p == position && *i == idx)
                                           || prev_chain.iter().any(|(p, i, _)| *p == position && *i == idx);

                        if already_used
                        {
                            continue;
                        }

                        // Update constraints with this YesSplit (like hard splits do)
                        let test_bit = 1u32 << idx;
                        let (new_constraints, _) = branch_constraints(
                            prev_constraints,
                            idx,
                            idx, // same as test_idx for YesSplits (hard splits)
                            position,
                            Some(test_bit), // yes branch allows this letter once
                            None, // no branch doesn't exist for YesSplit
                        );

                        // RE-SOLVE with updated constraints
                        let new_sol = solve(spec.no,
                                            ctx,
                                            allow_repeat,
                                            prioritize_soft_no,
                                            redeeming_yes,
                                            new_constraints,
                                            memo);

                        // Check if this YesSplit produced a bad/unsolvable result
                        if new_sol.is_unsolvable()
                        {
                            continue;
                        }

                        // Build the YesSplit chain
                        let mut new_chain = prev_chain.clone();
                        new_chain.push((position, idx, letter));
                        let yes_split_count = new_chain.len() as u32;

                        // Calculate cost for this YesSplit combination
                        let mut no_cost = add_no_edge(&new_sol.cost, spec.is_hard, redeeming_yes as i32);

                        // Apply YesSplit cost adjustments (-1 per YesSplit)
                        for _ in 0..yes_split_count
                        {
                            no_cost = add_yes_split(&no_cost);
                        }

                        // Add word_count contributions to no_cost sum metrics
                        no_cost.sum_nos += new_sol.cost.word_count;
                        no_cost.redeemed_sum_nos += new_sol.cost.word_count as i32 * redeeming_yes as i32;
                        if spec.is_hard
                        {
                            no_cost.sum_hard_nos += new_sol.cost.word_count;
                            no_cost.redeemed_sum_hard_nos +=
                                new_sol.cost.word_count as i32 * redeeming_yes as i32;
                        }

                        // Cap redeemed costs to not be negative
                        no_cost.redeemed_hard_nos = no_cost.redeemed_hard_nos.max(0);
                        no_cost.redeemed_nos = no_cost.redeemed_nos.max(0);
                        no_cost.redeemed_sum_hard_nos = no_cost.redeemed_sum_hard_nos.max(0);
                        no_cost.redeemed_sum_nos = no_cost.redeemed_sum_nos.max(0);

                        let yes_cost = add_yes_split(&yes_sol.cost);
                        let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
                        let redeemed_hard_nos = yes_cost.redeemed_hard_nos.max(no_cost.redeemed_hard_nos);
                        let nos = yes_cost.nos.max(no_cost.nos);
                        let redeemed_nos = yes_cost.redeemed_nos.max(no_cost.redeemed_nos);
                        let total_sum_nos = yes_sol.cost.sum_nos + no_cost.sum_nos;
                        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_cost.sum_hard_nos;
                        let total_redeemed_sum_nos = yes_sol.cost.redeemed_sum_nos + no_cost.redeemed_sum_nos;
                        let total_redeemed_sum_hard_nos =
                            yes_sol.cost.redeemed_sum_hard_nos + no_cost.redeemed_sum_hard_nos;

                        let branch_cost = Cost { hard_nos,
                                                 redeemed_hard_nos,
                                                 nos,
                                                 redeemed_nos,
                                                 sum_hard_nos: total_sum_hard_nos,
                                                 redeemed_sum_hard_nos: total_redeemed_sum_hard_nos,
                                                 sum_nos: total_sum_nos,
                                                 redeemed_sum_nos: total_redeemed_sum_nos,
                                                 word_count: yes_sol.cost.word_count
                                                             + new_sol.cost.word_count };

                        // Check if this solution is competitive with current best
                        let should_skip = if let Some(ref current_best) = best_cost
                        {
                            compare_costs(&branch_cost, current_best, prioritize_soft_no) == Ordering::Greater
                        }
                        else
                        {
                            false
                        };

                        if should_skip
                        {
                            continue;
                        }

                        // Add to next layer for further exploration
                        next_layer.push((new_sol.clone(), new_constraints, new_chain.clone()));

                        // Update best solution with this YesSplit combination
                        for tree in &new_sol.trees
                        {
                            // Build the YesSplit chain wrapping this tree
                            let mut wrapped_tree = Rc::clone(tree);
                            for (ys_pos, _, ys_letter) in &new_chain
                            {
                                wrapped_tree = combine_yes_split(*ys_letter,
                                                                 *ys_pos,
                                                                 *ys_letter,
                                                                 *ys_pos,
                                                                 &wrapped_tree);
                            }

                            // Combine with yes branch to form complete tree
                            match best_cost
                            {
                                None =>
                                {
                                    best_cost = Some(branch_cost);
                                    for y in &yes_sol.trees
                                    {
                                        best_trees.push(combine_positional_split(spec.test_letter,
                                                                                 spec.test_position,
                                                                                 spec.req_letter,
                                                                                 spec.req_position,
                                                                                 y,
                                                                                 &wrapped_tree));
                                    }
                                }
                                Some(ref current) =>
                                {
                                    match compare_costs(&branch_cost, current, prioritize_soft_no)
                                    {
                                        Ordering::Less =>
                                        {
                                            best_trees.clear();
                                            best_cost = Some(branch_cost);
                                            for y in &yes_sol.trees
                                            {
                                                best_trees.push(combine_positional_split(spec.test_letter,
                                                                                         spec.test_position,
                                                                                         spec.req_letter,
                                                                                         spec.req_position,
                                                                                         y,
                                                                                         &wrapped_tree));
                                            }
                                        }
                                        Ordering::Equal =>
                                        {
                                            for y in &yes_sol.trees
                                            {
                                                best_trees.push(combine_positional_split(spec.test_letter,
                                                                                         spec.test_position,
                                                                                         spec.req_letter,
                                                                                         spec.req_position,
                                                                                         y,
                                                                                         &wrapped_tree));
                                            }
                                        }
                                        Ordering::Greater =>
                                        {}
                                    }
                                }
                            }
                        }
                    }
                }

                // Move to next layer (or stop if no improvements found)
                if next_layer.is_empty()
                {
                    break;
                }
                current_layer = next_layer;
            }
        }
    }

    let sol = if let Some(cost) = best_cost
    {
        Solution { cost, trees: best_trees.into_vec() }
    }
    else
    {
        Solution::unsolvable(mask_count(mask))
    };
    memo.insert(key, sol.clone());
    sol
}
