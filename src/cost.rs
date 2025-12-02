use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost
{
    /// Number of hard No-edges on the heaviest path (primary objective).
    pub hard_nos: u32,
    /// Redeemed hard No-edges (scaled by redeeming_yes parameter).
    pub redeemed_hard_nos: i32,
    /// Number of No-edges on the heaviest path (secondary objective).
    pub nos: u32,
    /// Redeemed No-edges (scaled by redeeming_yes parameter).
    pub redeemed_nos: i32,
    /// Sum of hard No-edges weighted by word count (tertiary objective).
    pub sum_hard_nos: u32,
    /// Redeemed sum of hard No-edges (scaled by redeeming_yes parameter).
    pub redeemed_sum_hard_nos: i32,
    /// Sum of No-edges weighted by word count (quaternary objective).
    pub sum_nos: u32,
    /// Redeemed sum of No-edges (scaled by redeeming_yes parameter).
    pub redeemed_sum_nos: i32,
    /// Number of words in this subtree.
    pub word_count: u32
}

pub fn compare_costs(a: &Cost, b: &Cost, prioritize_soft_no: bool) -> Ordering
{
    if prioritize_soft_no
    {
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
    }
    else
    {
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
