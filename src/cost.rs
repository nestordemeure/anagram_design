use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    /// Number of hard No-edges on the heaviest path (primary objective).
    pub hard_nos: u32,
    /// Number of No-edges on the heaviest path (secondary objective).
    pub nos: u32,
    /// Sum of hard No-edges weighted by word count (tertiary objective).
    pub sum_hard_nos: u32,
    /// Sum of No-edges weighted by word count (quaternary objective).
    pub sum_nos: u32,
    /// Total depth (edges) on that path (quinary tie-breaker).
    pub depth: u32,
    /// Number of words in this subtree.
    pub word_count: u32,
}

pub fn compare_costs(a: &Cost, b: &Cost, prioritize_soft_no: bool) -> Ordering {
    if prioritize_soft_no {
        return a
            .hard_nos
            .cmp(&b.hard_nos)
            .then_with(|| a.nos.cmp(&b.nos))
            .then_with(|| a.sum_hard_nos.cmp(&b.sum_hard_nos))
            .then_with(|| a.sum_nos.cmp(&b.sum_nos))
            .then_with(|| a.depth.cmp(&b.depth));
    }

    // Average-based ordering: (max no, max hard no, avg no, avg hard no, depth)
    a.nos
        .cmp(&b.nos)
        .then_with(|| a.hard_nos.cmp(&b.hard_nos))
        .then_with(|| {
            let left = (a.sum_nos as u64) * (b.word_count as u64);
            let right = (b.sum_nos as u64) * (a.word_count as u64);
            left.cmp(&right)
        })
        .then_with(|| {
            let left = (a.sum_hard_nos as u64) * (b.word_count as u64);
            let right = (b.sum_hard_nos as u64) * (a.word_count as u64);
            left.cmp(&right)
        })
        .then_with(|| a.depth.cmp(&b.depth))
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hard_nos
            .cmp(&other.hard_nos)
            .then_with(|| self.nos.cmp(&other.nos))
            .then_with(|| self.sum_hard_nos.cmp(&other.sum_hard_nos))
            .then_with(|| self.sum_nos.cmp(&other.sum_nos))
            .then_with(|| self.depth.cmp(&other.depth))
    }
}

impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
