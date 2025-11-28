use hashbrown::HashMap;

use crate::node::Solution;
use crate::context::Context;
use crate::constraints::Constraints;
use crate::solver::solve;

pub fn minimal_trees(words: &[String], allow_repeat: bool, prioritize_soft_no: bool) -> Solution {
    // Default to keeping at most 5 optimal trees, matching the CLI display cap.
    minimal_trees_limited(words, allow_repeat, prioritize_soft_no, Some(5))
}

pub fn minimal_trees_limited(
    words: &[String],
    allow_repeat: bool,
    prioritize_soft_no: bool,
    limit: Option<usize>,
) -> Solution {
    assert!(words.len() <= 16, "bitmask solver supports up to 16 words");
    let ctx = Context::new(words);
    let mask = if words.len() == 16 {
        u16::MAX
    } else {
        (1u16 << words.len()) - 1
    };
    let mut memo = HashMap::new();
    solve(
        mask,
        &ctx,
        allow_repeat,
        prioritize_soft_no,
        Constraints::empty(),
        limit,
        &mut memo,
    )
}
