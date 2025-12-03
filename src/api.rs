use hashbrown::HashMap;

use crate::node::Solution;
use crate::context::{Context, Mask};
use crate::constraints::Constraints;
use crate::dijkstra_solver::solve;

/// Compute all optimal trees for the given word list.
pub fn minimal_trees(
    words: &[String],
    allow_repeat: bool,
    prioritize_soft_no: bool,
    redeeming_yes: u32,
) -> Solution {
    assert!(words.len() <= 32, "bitmask solver supports up to 32 words");
    let ctx = Context::new(words);
    let mask = if words.len() == 32 { Mask::MAX } else { ((1 as Mask) << words.len()) - 1 };
    let mut memo = HashMap::new();
    solve(mask, &ctx, allow_repeat, prioritize_soft_no, redeeming_yes, Constraints::empty(), &mut memo)
}
