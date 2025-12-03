use std::cmp::Ordering;
use crate::context::{Mask, mask_count};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    /// Number of hard No-edges on the heaviest path (primary objective).
    pub hard_nos: u32,
    /// Redeemed hard No-edges (scaled by `redeeming_yes` parameter).
    pub redeemed_hard_nos: i32,
    /// Number of No-edges on the heaviest path (secondary objective).
    pub nos: u32,
    /// Redeemed No-edges (scaled by `redeeming_yes` parameter).
    pub redeemed_nos: i32,
    /// Sum of hard No-edges weighted by word count (tertiary objective).
    pub sum_hard_nos: u32,
    /// Redeemed sum of hard No-edges (scaled by `redeeming_yes` parameter).
    pub redeemed_sum_hard_nos: i32,
    /// Sum of No-edges weighted by word count (quaternary objective).
    pub sum_nos: u32,
    /// Redeemed sum of No-edges (scaled by `redeeming_yes` parameter).
    pub redeemed_sum_nos: i32,
    /// Number of words in this subtree.
    pub word_count: u32,
}

/// Increment a cost by adding a no-edge.
/// Used when traversing the no branch of a split.
pub fn add_no_edge(base: &Cost, is_hard: bool, redeeming_yes: i32) -> Cost {
    if is_hard {
        Cost {
            hard_nos: base.hard_nos + 1,
            redeemed_hard_nos: base.redeemed_hard_nos + redeeming_yes,
            nos: base.nos + 1,
            redeemed_nos: base.redeemed_nos + redeeming_yes,
            sum_hard_nos: base.sum_hard_nos,
            redeemed_sum_hard_nos: base.redeemed_sum_hard_nos,
            sum_nos: base.sum_nos,
            redeemed_sum_nos: base.redeemed_sum_nos,
            word_count: base.word_count,
        }
    } else {
        Cost {
            hard_nos: base.hard_nos,
            redeemed_hard_nos: base.redeemed_hard_nos,
            nos: base.nos + 1,
            redeemed_nos: base.redeemed_nos + redeeming_yes,
            sum_hard_nos: base.sum_hard_nos,
            redeemed_sum_hard_nos: base.redeemed_sum_hard_nos,
            sum_nos: base.sum_nos,
            redeemed_sum_nos: base.redeemed_sum_nos,
            word_count: base.word_count,
        }
    }
}

/// Apply the cost impact of a `YesSplit`.
/// `YesSplit`s contribute -1 to all `redeemed_*` metrics (not scaled by `redeeming_yes`).
pub fn add_yes_split(base: &Cost) -> Cost {
    Cost {
        hard_nos: base.hard_nos,
        redeemed_hard_nos: base.redeemed_hard_nos - 1,
        nos: base.nos,
        redeemed_nos: base.redeemed_nos - 1,
        sum_hard_nos: base.sum_hard_nos,
        redeemed_sum_hard_nos: base.redeemed_sum_hard_nos - 1,
        sum_nos: base.sum_nos,
        redeemed_sum_nos: base.redeemed_sum_nos - 1,
        word_count: base.word_count,
    }
}

/// Estimate lower bound cost for a state (used for candidate ordering).
/// This provides an optimistic (lower) bound that guarantees we won't prune optimal solutions.
pub fn estimate_cost(mask: Mask, allow_repeat: bool, redeeming_yes: u32) -> Cost {
    // Lower bounds:
    // - nos: 1 if N >= threshold, else 0
    //   - When allow_repeat=true: threshold is 3 (2 words can be handled with Repeat, nos=0)
    //   - When allow_repeat=false: threshold is 2 (need at least one split)
    // - hard_nos: 0 (optimistic: assume all soft splits)
    // - sum_nos: N-1 (balanced tree has N-1 internal nodes, each adds ≥1)
    // - sum_hard_nos: 0 (optimistic: assume all soft)
    let count: u32 = mask_count(mask);
    let threshold = if allow_repeat { 3 } else { 2 };
    let nos_estimate = if count >= threshold { 1 } else { 0 };
    let sum_nos_estimate = count.saturating_sub(1);

    // the `nos_estimate * redeeming_yes` redemed costs are actualy pessimistic, but necessary to avoid paths explosions
    Cost {
        hard_nos: 0, // Optimistic: all soft
        redeemed_hard_nos: 0,
        nos: nos_estimate,
        redeemed_nos: (nos_estimate * redeeming_yes) as i32,
        sum_hard_nos: 0, // Optimistic: all soft
        redeemed_sum_hard_nos: 0,
        sum_nos: sum_nos_estimate,
        redeemed_sum_nos: (sum_nos_estimate * redeeming_yes) as i32,
        word_count: count,
    }
}

pub fn compare_costs(a: &Cost, b: &Cost, prioritize_soft_no: bool) -> Ordering {
    if prioritize_soft_no {
        a.redeemed_hard_nos
            .cmp(&b.redeemed_hard_nos)
            .then_with(|| a.hard_nos.cmp(&b.hard_nos))
            .then_with(|| {
                let left = (a.redeemed_sum_hard_nos as i64) * (b.word_count as i64);
                let right = (b.redeemed_sum_hard_nos as i64) * (a.word_count as i64);
                left.cmp(&right)
            })
            .then_with(|| {
                let left = (a.sum_hard_nos as u64) * (b.word_count as u64);
                let right = (b.sum_hard_nos as u64) * (a.word_count as u64);
                left.cmp(&right)
            })
            .then_with(|| a.redeemed_nos.cmp(&b.redeemed_nos))
            .then_with(|| a.nos.cmp(&b.nos))
            .then_with(|| {
                let left = (a.redeemed_sum_nos as i64) * (b.word_count as i64);
                let right = (b.redeemed_sum_nos as i64) * (a.word_count as i64);
                left.cmp(&right)
            })
            .then_with(|| {
                let left = (a.sum_nos as u64) * (b.word_count as u64);
                let right = (b.sum_nos as u64) * (a.word_count as u64);
                left.cmp(&right)
            })
    } else {
        a.redeemed_nos
            .cmp(&b.redeemed_nos)
            .then_with(|| a.nos.cmp(&b.nos))
            .then_with(|| {
                let left = (a.redeemed_sum_nos as i64) * (b.word_count as i64);
                let right = (b.redeemed_sum_nos as i64) * (a.word_count as i64);
                left.cmp(&right)
            })
            .then_with(|| {
                let left = (a.sum_nos as u64) * (b.word_count as u64);
                let right = (b.sum_nos as u64) * (a.word_count as u64);
                left.cmp(&right)
            })
            .then_with(|| a.redeemed_hard_nos.cmp(&b.redeemed_hard_nos))
            .then_with(|| a.hard_nos.cmp(&b.hard_nos))
            .then_with(|| {
                let left = (a.redeemed_sum_hard_nos as i64) * (b.word_count as i64);
                let right = (b.redeemed_sum_hard_nos as i64) * (a.word_count as i64);
                left.cmp(&right)
            })
            .then_with(|| {
                let left = (a.sum_hard_nos as u64) * (b.word_count as u64);
                let right = (b.sum_hard_nos as u64) * (a.word_count as u64);
                left.cmp(&right)
            })
    }
}
