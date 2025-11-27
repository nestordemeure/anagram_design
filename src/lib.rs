use std::cmp::Ordering;
use std::collections::HashMap;

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

fn compare_costs(a: &Cost, b: &Cost, prioritize_soft_no: bool) -> Ordering {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Leaf(String),
    Repeat(String, String),
    Split {
        letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
    SoftSplit {
        /// Letter to test for (e.g., 'i' in I/E)
        test_letter: char,
        /// Letter that all No items must contain (e.g., 'e' in I/E)
        requirement_letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
    FirstLetterSplit {
        letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
    SoftFirstLetterSplit {
        /// Letter to test as first letter
        test_letter: char,
        /// Letter that all No items must have as second letter
        requirement_letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
    LastLetterSplit {
        letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
    SoftLastLetterSplit {
        /// Letter to test as last letter
        test_letter: char,
        /// Letter that all No items must have as second-to-last letter
        requirement_letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
    SoftMirrorPosSplit {
        /// Letter to test
        test_letter: char,
        /// 1-based position from the tested end (1 = first/last)
        test_index: u8,
        /// true when counting from the end (last/second-to-last/third-to-last)
        test_from_end: bool,
        /// Position that all No items must carry the same letter in
        requirement_index: u8,
        /// true when the requirement position is counted from the end
        requirement_from_end: bool,
        yes: Box<Node>,
        no: Box<Node>,
    },
    SoftDoubleLetterSplit {
        /// Letter that must appear twice in the Yes branch
        test_letter: char,
        /// Letter (different) that must appear twice in all No items
        requirement_letter: char,
        yes: Box<Node>,
        no: Box<Node>,
    },
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub cost: Cost,
    pub trees: Vec<Node>,
    pub exhausted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    mask: u16,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    /// Bitmask of known letters (letters we know exist in all words in this branch)
    known_letters: u32,
}

/// Defines a soft no pair: (test_letter, requirement_letter)
/// E/I means: test for 'e', require all No items contain 'i'
/// Children cannot use any soft no containing either letter
#[derive(Debug, Clone, Copy)]
struct SoftNoPair {
    /// Test for this letter
    test_letter: char,
    /// Require all No items contain this letter
    requirement_letter: char,
}

struct Context<'a> {
    words: &'a [String],
    letter_masks: [u16; 26],
    first_letter_masks: [u16; 26],
    second_letter_masks: [u16; 26],
    third_letter_masks: [u16; 26],
    last_letter_masks: [u16; 26],
    second_to_last_letter_masks: [u16; 26],
    third_to_last_letter_masks: [u16; 26],
    double_letter_masks: [u16; 26],
}

/// Define the available soft no pairs
/// Children of a soft no cannot use any soft no containing either letter
const SOFT_NO_PAIRS: &[SoftNoPair] = &[
    // E/I pair - vowel similarity
    SoftNoPair { test_letter: 'e', requirement_letter: 'i' },
    SoftNoPair { test_letter: 'i', requirement_letter: 'e' },

    // C/K pair - identical hard sound
    SoftNoPair { test_letter: 'c', requirement_letter: 'k' },
    SoftNoPair { test_letter: 'k', requirement_letter: 'c' },

    // S/Z pair - similar sibilants
    SoftNoPair { test_letter: 's', requirement_letter: 'z' },
    SoftNoPair { test_letter: 'z', requirement_letter: 's' },

    // I/L pair - visually similar
    SoftNoPair { test_letter: 'i', requirement_letter: 'l' },
    SoftNoPair { test_letter: 'l', requirement_letter: 'i' },

    // M/N pair - nasals
    SoftNoPair { test_letter: 'm', requirement_letter: 'n' },
    SoftNoPair { test_letter: 'n', requirement_letter: 'm' },

    // U/V pair - visually similar
    SoftNoPair { test_letter: 'u', requirement_letter: 'v' },
    SoftNoPair { test_letter: 'v', requirement_letter: 'u' },

    // O/Q pair - visually similar
    SoftNoPair { test_letter: 'o', requirement_letter: 'q' },
    SoftNoPair { test_letter: 'q', requirement_letter: 'o' },

    // C/G pair - visually similar
    SoftNoPair { test_letter: 'c', requirement_letter: 'g' },
    SoftNoPair { test_letter: 'g', requirement_letter: 'c' },

    // B/P pair - voiced/unvoiced
    SoftNoPair { test_letter: 'b', requirement_letter: 'p' },
    SoftNoPair { test_letter: 'p', requirement_letter: 'b' },

    // I/T pair - visually similar
    SoftNoPair { test_letter: 'i', requirement_letter: 't' },
    SoftNoPair { test_letter: 't', requirement_letter: 'i' },

    // R/E pair
    SoftNoPair { test_letter: 'r', requirement_letter: 'e' },
    SoftNoPair { test_letter: 'e', requirement_letter: 'r' },

    // A/R pair - similar open shapes in block capitals
    SoftNoPair { test_letter: 'a', requirement_letter: 'r' },
    SoftNoPair { test_letter: 'r', requirement_letter: 'a' },
];

fn mask_count(mask: u16) -> u32 {
    mask.count_ones()
}

fn position_mask(ctx: &Context<'_>, from_end: bool, pos_index: u8, letter_idx: usize) -> u16 {
    match (from_end, pos_index) {
        (false, 1) => ctx.first_letter_masks[letter_idx],
        (false, 2) => ctx.second_letter_masks[letter_idx],
        (false, 3) => ctx.third_letter_masks[letter_idx],
        (true, 1) => ctx.last_letter_masks[letter_idx],
        (true, 2) => ctx.second_to_last_letter_masks[letter_idx],
        (true, 3) => ctx.third_to_last_letter_masks[letter_idx],
        _ => 0,
    }
}

fn single_word_from_mask(mask: u16, words: &[String]) -> Option<String> {
    let idx = mask.trailing_zeros() as usize;
    if idx < words.len() {
        Some(words[idx].clone())
    } else {
        None
    }
}

/// Return all letter indices that produce a true partition of `mask` with the given per-letter masks.
/// Each item is (letter_index, yes_mask, no_mask).
fn partitions(mask: u16, masks: &[u16; 26]) -> Vec<(usize, u16, u16)> {
    masks
        .iter()
        .enumerate()
        .filter_map(|(idx, &letter_mask)| {
            let yes = mask & letter_mask;
            if yes == 0 || yes == mask {
                None
            } else {
                Some((idx, yes, mask & !letter_mask))
            }
        })
        .collect()
}

fn two_words_from_mask(mask: u16, words: &[String]) -> Option<(String, String)> {
    if mask_count(mask) != 2 {
        return None;
    }
    let first = mask.trailing_zeros() as usize;
    let second_mask = mask & !(1u16 << first);
    let second = second_mask.trailing_zeros() as usize;
    if second < words.len() {
        Some((words[first].clone(), words[second].clone()))
    } else {
        None
    }
}

fn combine_children(letter: char, left: &Node, right: &Node) -> Node {
    Node::Split {
        letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_soft_children(test_letter: char, requirement_letter: char, left: &Node, right: &Node) -> Node {
    Node::SoftSplit {
        test_letter,
        requirement_letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_first_letter_children(letter: char, left: &Node, right: &Node) -> Node {
    Node::FirstLetterSplit {
        letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_soft_first_letter_children(test_letter: char, requirement_letter: char, left: &Node, right: &Node) -> Node {
    Node::SoftFirstLetterSplit {
        test_letter,
        requirement_letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_last_letter_children(letter: char, left: &Node, right: &Node) -> Node {
    Node::LastLetterSplit {
        letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_soft_last_letter_children(test_letter: char, requirement_letter: char, left: &Node, right: &Node) -> Node {
    Node::SoftLastLetterSplit {
        test_letter,
        requirement_letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_soft_mirror_pos_children(
    test_letter: char,
    test_index: u8,
    test_from_end: bool,
    requirement_index: u8,
    requirement_from_end: bool,
    left: &Node,
    right: &Node,
) -> Node {
    Node::SoftMirrorPosSplit {
        test_letter,
        test_index,
        test_from_end,
        requirement_index,
        requirement_from_end,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn combine_soft_double_letter_children(test_letter: char, requirement_letter: char, left: &Node, right: &Node) -> Node {
    Node::SoftDoubleLetterSplit {
        test_letter,
        requirement_letter,
        yes: Box::new(left.clone()),
        no: Box::new(right.clone()),
    }
}

fn push_limited(target: &mut Vec<Node>, limit: Option<usize>, node: Node) -> bool {
    match limit {
        Some(max) if target.len() >= max => false,
        _ => {
            target.push(node);
            true
        }
    }
}

fn solve(
    mask: u16,
    ctx: &Context<'_>,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    known_letters: u32,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let key = Key { mask, allow_repeat, prioritize_soft_no, known_letters };
    if let Some(hit) = memo.get(&key) {
        return hit.clone();
    }

    let count = mask_count(mask);

    // Leaf node
    if count == 1 {
        let word = single_word_from_mask(mask, ctx.words).expect("mask must map to a word");
        let sol = Solution {
            cost: Cost { nos: 0, hard_nos: 0, sum_nos: 0, sum_hard_nos: 0, depth: 0, word_count: 1 },
            trees: vec![Node::Leaf(word)],
            exhausted: false,
        };
        memo.insert(key, sol.clone());
        return sol;
    }

    let mut best_cost: Option<Cost> = None;
    let mut best_trees: Vec<Node> = Vec::new();
    let mut exhausted = false;

    // Repeat node option for exactly two words
    if allow_repeat && count == 2 {
        if let Some((w1, w2)) = two_words_from_mask(mask, ctx.words) {
            best_cost = Some(Cost { nos: 0, hard_nos: 0, sum_nos: 0, sum_hard_nos: 0, depth: 0, word_count: 2 });
            if !push_limited(&mut best_trees, limit, Node::Repeat(w1, w2)) {
                exhausted = true;
            }
        }
    }

    for (idx, yes, no) in partitions(mask, &ctx.letter_masks) {
        // In the YES branch, we know this letter exists in all words
        let letter_bit = 1u32 << idx;
        let yes_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, known_letters, limit, memo);

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
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
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

        let yes = mask & ctx.letter_masks[test_idx];
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !ctx.letter_masks[test_idx];

        // Check if all items in the "no" set contain the requirement letter
        if no & ctx.letter_masks[requirement_idx] != no {
            continue; // not all No items contain the requirement letter
        }

        // In the YES branch, we know the test_letter exists; in the NO branch, we know the requirement_letter exists
        let yes_known = known_letters | test_bit;
        let no_known = known_letters | requirement_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, no_known, limit, memo);

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
                Ordering::Equal => {
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
                Ordering::Greater => {}
            },
        }
    }

    // Soft double-letter splits: Yes has two of test_letter; No has two of a different uniform letter
    for (test_idx, yes, no) in partitions(mask, &ctx.double_letter_masks) {
        let test_bit = 1u32 << test_idx;

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

        let requirement_bit = 1u32 << requirement_idx;

        let yes_known = known_letters | test_bit;
        let no_known = known_letters | requirement_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, no_known, limit, memo);

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
                            combine_soft_double_letter_children(test_letter, requirement_letter, y, n),
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
                                combine_soft_double_letter_children(test_letter, requirement_letter, y, n),
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
                                combine_soft_double_letter_children(test_letter, requirement_letter, y, n),
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
        // In the YES branch, we know this letter exists in all words (as first letter)
        let letter_bit = 1u32 << idx;
        let yes_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, known_letters, limit, memo);

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
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
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
        let letter_bit = 1u32 << idx;

        // Check if all items in the "no" set have the same letter as second letter
        if no & ctx.second_letter_masks[idx] != no {
            continue;
        }

        // In YES branch, we know letter is first; in NO branch, we know letter is second
        // Either way, the letter exists in all words
        let child_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);

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
            let letter_bit = 1u32 << idx;
            let yes = mask & position_mask(ctx, false, pos, idx);
            if yes == 0 || yes == mask {
                continue;
            }
            let no = mask & !position_mask(ctx, false, pos, idx);

            // All No items must carry the same letter in the mirrored-from-end position
            if no & position_mask(ctx, true, pos, idx) != no {
                continue;
            }

            let child_known = known_letters | letter_bit;
            let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);
            let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);

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
                                combine_soft_mirror_pos_children(letter, pos, false, pos, true, y, n),
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
                                    combine_soft_mirror_pos_children(letter, pos, false, pos, true, y, n),
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
                                    combine_soft_mirror_pos_children(letter, pos, false, pos, true, y, n),
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
            let letter_bit = 1u32 << idx;
            let yes = mask & position_mask(ctx, true, pos, idx);
            if yes == 0 || yes == mask {
                continue;
            }
            let no = mask & !position_mask(ctx, true, pos, idx);

            // All No items must carry the same letter in the mirrored-from-start position
            if no & position_mask(ctx, false, pos, idx) != no {
                continue;
            }

            let child_known = known_letters | letter_bit;
            let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);
            let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);

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
                                combine_soft_mirror_pos_children(letter, pos, true, pos, false, y, n),
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
                                    combine_soft_mirror_pos_children(letter, pos, true, pos, false, y, n),
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
                                    combine_soft_mirror_pos_children(letter, pos, true, pos, false, y, n),
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
        // In the YES branch, we know this letter exists in all words (as last letter)
        let letter_bit = 1u32 << idx;
        let yes_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, known_letters, limit, memo);

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
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
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
        let letter_bit = 1u32 << idx;

        // Check if all items in the "no" set have the same letter as second-to-last letter
        if no & ctx.second_to_last_letter_masks[idx] != no {
            continue;
        }

        // In YES branch, we know letter is last; in NO branch, we know letter is second-to-last
        // Either way, the letter exists in all words
        let child_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, prioritize_soft_no, child_known, limit, memo);

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
        trees: best_trees,
        exhausted,
    };
    memo.insert(key, sol.clone());
    sol
}

fn make_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_first_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().next() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_second_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().nth(1) {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_third_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().nth(2) {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_last_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().last() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_second_to_last_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let chars: Vec<char> = w.chars().collect();
        if chars.len() >= 2 {
            let ch = chars[chars.len() - 2];
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_third_to_last_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let chars: Vec<char> = w.chars().collect();
        if chars.len() >= 3 {
            let ch = chars[chars.len() - 3];
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_double_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let mut counts = [0u8; 26];
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                if counts[l] < 2 {
                    counts[l] += 1;
                }
            }
        }
        for (l, &c) in counts.iter().enumerate() {
            if c >= 2 {
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

pub fn minimal_trees(words: &[String], allow_repeat: bool, prioritize_soft_no: bool) -> Solution {
    // Default to keeping at most 5 optimal trees, matching the CLI display cap.
    minimal_trees_limited(words, allow_repeat, prioritize_soft_no, Some(5))
}

pub fn minimal_trees_limited(words: &[String], allow_repeat: bool, prioritize_soft_no: bool, limit: Option<usize>) -> Solution {
    assert!(words.len() <= 16, "bitmask solver supports up to 16 words");
    let letter_masks = make_letter_masks(words);
    let first_letter_masks = make_first_letter_masks(words);
    let second_letter_masks = make_second_letter_masks(words);
    let third_letter_masks = make_third_letter_masks(words);
    let last_letter_masks = make_last_letter_masks(words);
    let second_to_last_letter_masks = make_second_to_last_letter_masks(words);
    let third_to_last_letter_masks = make_third_to_last_letter_masks(words);
    let double_letter_masks = make_double_letter_masks(words);
    let ctx = Context {
        words,
        letter_masks,
        first_letter_masks,
        second_letter_masks,
        third_letter_masks,
        last_letter_masks,
        second_to_last_letter_masks,
        third_to_last_letter_masks,
        double_letter_masks,
    };
    let mask = if words.len() == 16 { u16::MAX } else { (1u16 << words.len()) - 1 };
    let mut memo = HashMap::new();
    solve(mask, &ctx, allow_repeat, prioritize_soft_no, 0, limit, &mut memo)
}

pub fn format_tree(node: &Node) -> String {
    // Helper to capitalize the first letter of a word
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    fn describe_pos(from_end: bool, idx: u8) -> String {
        match (from_end, idx) {
            (false, 1) => "first".to_string(),
            (false, 2) => "second".to_string(),
            (false, 3) => "third".to_string(),
            (true, 1) => "last".to_string(),
            (true, 2) => "second-to-last".to_string(),
            (true, 3) => "third-to-last".to_string(),
            _ => format!("pos {}", idx),
        }
    }

    // Render a No branch that diverges sideways from the main spine.
    fn render_no_branch(node: &Node, prefix: &str, out: &mut String) {
        match node {
            Node::Leaf(w) => {
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&capitalize_first(w));
                out.push('\n');
            }
            Node::Repeat(a, b) => {
                out.push_str(prefix);
                out.push_str("└─ No: Repeat: ");
                out.push_str(&capitalize_first(a));
                out.push_str(" / ");
                out.push_str(&capitalize_first(b));
                out.push('\n');
            }
            Node::Split { letter, yes, no } => {
                // No branch that contains another split
                out.push_str(prefix);
                out.push_str("└─ No: Contains '");
                out.push(*letter);
                out.push_str("'?\n");

                // The no-branch's children are indented with "│   "
                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                // The yes branch of this nested split uses └─ (it's the final item in this branch)
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftSplit { test_letter, requirement_letter, yes, no } => {
                // No branch that contains a soft split
                out.push_str(prefix);
                out.push_str("└─ No: Contains '");
                out.push(*test_letter);
                out.push_str("'? (all No contain '");
                out.push(*requirement_letter);
                out.push_str("')\n");

                // The no-branch's children are indented with "│   "
                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                // The yes branch of this nested split uses └─ (it's the final item in this branch)
                render_yes_final(yes, &child_prefix, out);
            }
            Node::FirstLetterSplit { letter, yes, no } => {
                // No branch that contains a first letter split
                out.push_str(prefix);
                out.push_str("└─ No: First letter '");
                out.push(*letter);
                out.push_str("'?\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftFirstLetterSplit { test_letter, requirement_letter, yes, no } => {
                // No branch that contains a soft first letter split
                out.push_str(prefix);
                out.push_str("└─ No: First letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have '");
                out.push(*requirement_letter);
                out.push_str("' second)\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // No branch that contains a last letter split
                out.push_str(prefix);
                out.push_str("└─ No: Last letter '");
                out.push(*letter);
                out.push_str("'?\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftLastLetterSplit { test_letter, requirement_letter, yes, no } => {
                // No branch that contains a soft last letter split
                out.push_str(prefix);
                out.push_str("└─ No: Last letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have '");
                out.push(*requirement_letter);
                out.push_str("' second-to-last)\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftMirrorPosSplit { test_letter, test_index, test_from_end, requirement_index, requirement_from_end, yes, no } => {
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftDoubleLetterSplit { test_letter, requirement_letter, yes, no } => {
                out.push_str(prefix);
                out.push_str("└─ No: Double '");
                out.push(*test_letter);
                out.push_str("'? (all No double '");
                out.push(*requirement_letter);
                out.push_str("')\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
        }
    }

    // Render a final Yes item (uses └─ marker for leaves/repeats, continues spine for splits)
    fn render_yes_final(node: &Node, prefix: &str, out: &mut String) {
        match node {
            Node::Leaf(w) => {
                out.push_str(prefix);
                out.push_str("└─ ");
                out.push_str(&capitalize_first(w));
                out.push('\n');
            }
            Node::Repeat(a, b) => {
                out.push_str(prefix);
                out.push_str("└─ Repeat: ");
                out.push_str(&capitalize_first(a));
                out.push_str(" / ");
                out.push_str(&capitalize_first(b));
                out.push('\n');
            }
            Node::Split { letter, yes, no } => {
                // For a split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(*letter);
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftSplit { test_letter, requirement_letter, yes, no } => {
                // For a soft split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(*test_letter);
                out.push_str("'? (all No contain '");
                out.push(*requirement_letter);
                out.push_str("')\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::FirstLetterSplit { letter, yes, no } => {
                // For a first letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(*letter);
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftFirstLetterSplit { test_letter, requirement_letter, yes, no } => {
                // For a soft first letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have '");
                out.push(*requirement_letter);
                out.push_str("' second)\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // For a last letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(*letter);
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftLastLetterSplit { test_letter, requirement_letter, yes, no } => {
                // For a soft last letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have '");
                out.push(*requirement_letter);
                out.push_str("' second-to-last)\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftMirrorPosSplit { test_letter, test_index, test_from_end, requirement_index, requirement_from_end, yes, no } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftDoubleLetterSplit { test_letter, requirement_letter, yes, no } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Double '");
                out.push(*test_letter);
                out.push_str("'? (all No double '");
                out.push(*requirement_letter);
                out.push_str("')\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
        }
    }

    // Render the main Yes spine; No branches jut out to the side.
    fn render_spine(node: &Node, prefix: &str, is_final: bool, out: &mut String) {
        match node {
            Node::Leaf(w) => {
                let connector = if is_final { "└─ " } else { "├─ " };
                out.push_str(prefix);
                out.push_str(connector);
                out.push_str(&capitalize_first(w));
                out.push('\n');
            }
            Node::Repeat(a, b) => {
                let connector = if is_final { "└─ " } else { "├─ " };
                out.push_str(prefix);
                out.push_str(connector);
                out.push_str("Repeat: ");
                out.push_str(&capitalize_first(a));
                out.push_str(" / ");
                out.push_str(&capitalize_first(b));
                out.push('\n');
            }
            Node::Split { letter, yes, no } => {
                // Print the question
                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(*letter);
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftSplit { test_letter, requirement_letter, yes, no } => {
                // Print the question for soft split
                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(*test_letter);
                out.push_str("'? (all No contain '");
                out.push(*requirement_letter);
                out.push_str("')\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::FirstLetterSplit { letter, yes, no } => {
                // Print the question for first letter split
                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(*letter);
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftFirstLetterSplit { test_letter, requirement_letter, yes, no } => {
                // Print the question for soft first letter split
                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have '");
                out.push(*requirement_letter);
                out.push_str("' second)\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // Print the question for last letter split
                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(*letter);
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftLastLetterSplit { test_letter, requirement_letter, yes, no } => {
                // Print the question for soft last letter split
                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have '");
                out.push(*requirement_letter);
                out.push_str("' second-to-last)\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftMirrorPosSplit { test_letter, test_index, test_from_end, requirement_index, requirement_from_end, yes, no } => {
                out.push_str(prefix);
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(*test_letter);
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftDoubleLetterSplit { test_letter, requirement_letter, yes, no } => {
                out.push_str(prefix);
                out.push_str("Double '");
                out.push(*test_letter);
                out.push_str("'? (all No double '");
                out.push(*requirement_letter);
                out.push_str("')\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(yes, prefix, is_final, out);
            }
        }
    }

    let mut out = String::new();
    render_spine(node, "", true, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
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

        assert_eq!(compare_costs(&soft_first, &hard_first, true), Ordering::Less);
        assert_eq!(compare_costs(&soft_first, &hard_first, false), Ordering::Greater);
    }

    /// Recompute the full Cost of a tree by walking it, independent of the solver.
    fn compute_cost(node: &Node) -> Cost {
        match node {
            Node::Leaf(_) => Cost { nos: 0, hard_nos: 0, sum_nos: 0, sum_hard_nos: 0, depth: 0, word_count: 1 },
            Node::Repeat(_, _) => Cost { nos: 0, hard_nos: 0, sum_nos: 0, sum_hard_nos: 0, depth: 0, word_count: 2 },
            Node::Split { yes, no, .. }
            | Node::FirstLetterSplit { yes, no, .. }
            | Node::LastLetterSplit { yes, no, .. } => {
                let yes_cost = compute_cost(yes);
                let no_cost_base = compute_cost(no);
                let no_cost = Cost {
                    nos: no_cost_base.nos + 1,
                    hard_nos: no_cost_base.hard_nos + 1,
                    sum_nos: no_cost_base.sum_nos,
                    sum_hard_nos: no_cost_base.sum_hard_nos,
                    depth: no_cost_base.depth,
                    word_count: no_cost_base.word_count,
                };
                let nos = yes_cost.nos.max(no_cost.nos);
                let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
                let depth = yes_cost.depth.max(no_cost.depth) + 1;
                let sum_nos = yes_cost.sum_nos + no_cost.sum_nos + no_cost.word_count;
                let sum_hard_nos = yes_cost.sum_hard_nos + no_cost.sum_hard_nos + no_cost.word_count;
                Cost {
                    nos,
                    hard_nos,
                    sum_nos,
                    sum_hard_nos,
                    depth,
                    word_count: yes_cost.word_count + no_cost.word_count,
                }
            }
            Node::SoftSplit { yes, no, .. }
            | Node::SoftFirstLetterSplit { yes, no, .. }
            | Node::SoftLastLetterSplit { yes, no, .. }
            | Node::SoftMirrorPosSplit { yes, no, .. }
            | Node::SoftDoubleLetterSplit { yes, no, .. } => {
                let yes_cost = compute_cost(yes);
                let no_cost_base = compute_cost(no);
                let no_cost = Cost {
                    nos: no_cost_base.nos + 1,
                    hard_nos: no_cost_base.hard_nos,
                    sum_nos: no_cost_base.sum_nos,
                    sum_hard_nos: no_cost_base.sum_hard_nos,
                    depth: no_cost_base.depth,
                    word_count: no_cost_base.word_count,
                };
                let nos = yes_cost.nos.max(no_cost.nos);
                let hard_nos = yes_cost.hard_nos.max(no_cost.hard_nos);
                let depth = yes_cost.depth.max(no_cost.depth) + 1;
                let sum_nos = yes_cost.sum_nos + no_cost.sum_nos + no_cost.word_count;
                let sum_hard_nos = yes_cost.sum_hard_nos + no_cost.sum_hard_nos;
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
        assert!(matches!(with_repeat.trees[0], Node::Repeat(_, _)));
    }

    #[test]
    fn simple_split_cost() {
        let data = words(&["ab", "ac", "b"]);
        let sol = minimal_trees(&data, false, true);
        assert_eq!(sol.cost, Cost { nos: 1, hard_nos: 1, sum_nos: 2, sum_hard_nos: 1, depth: 2, word_count: 3 });
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, true, Some(1));
        // Regression test: R/E pair + C/G pair enable all-soft-test trees
        // With R/E and C/G soft pairs, Virgo/Scorpio can be separated by:
        //   1. "Contains 'r'? (all No contain 'e')" - soft
        //   2. "Contains 'c'? (all No contain 'g')" - soft (Scorpio has c, Virgo has g)
        // This achieves hard_nos: 0!
        assert_eq!(allow_repeat.cost, Cost { nos: 3, hard_nos: 0, sum_nos: 16, sum_hard_nos: 0, depth: 5, word_count: 12 });
        assert_eq!(no_repeat.cost, Cost { nos: 3, hard_nos: 0, sum_nos: 20, sum_hard_nos: 0, depth: 7, word_count: 12 });
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
        assert_eq!(sol.cost.hard_nos, 0, "Expected 0 hard NOs (all soft), got {}", sol.cost.hard_nos);
    }

    #[test]
    fn soft_known_letter_pruning_regression() {
        // Under the written rules, two nested soft tests should give 0 hard NOs:
        //   1) Contains 'r'? (all No contain 'e')
        //   2) In the Yes branch, Contains 't'? (all No contain 'r')
        // Current solver skips step (2) because 'r' is already known, so it returns hard_nos = 1.
        let data = words(&["tr", "r", "e"]);
        let sol = minimal_trees(&data, false, true);
        assert_eq!(
            sol.cost,
            Cost { hard_nos: 0, nos: 1, sum_hard_nos: 0, sum_nos: 2, depth: 2, word_count: 3 },
            "Expected fully-soft separation with 0 hard NOs; got {:?}",
            sol.cost
        );
    }

    #[test]
    fn recomputed_cost_matches_expected_for_top_tree() {
        // Use the first printed allow_repeat tree to assert its true hard_no count.
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let sol = minimal_trees_limited(&data, true, true, Some(1));
        let tree = &sol.trees[0];
        let cost = compute_cost(tree);
        assert_eq!(cost, Cost { nos: 3, hard_nos: 0, sum_nos: 16, sum_hard_nos: 0, depth: 5, word_count: 12 });
    }

    #[test]
    fn solver_advertised_cost_matches_tree_cost_allow_repeat() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let sol = minimal_trees_limited(&data, true, true, Some(3));
        for (idx, tree) in sol.trees.iter().take(3).enumerate() {
            let tree_cost = compute_cost(tree);
            assert_eq!(
                sol.cost, tree_cost,
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
        let sol = minimal_trees_limited(&data, true, true, Some(1));
        assert_eq!(
            sol.cost,
            Cost { nos: 1, hard_nos: 0, sum_nos: 2, sum_hard_nos: 0, depth: 1, word_count: 4 }
        );
        match &sol.trees[0] {
            Node::SoftDoubleLetterSplit { test_letter, requirement_letter, yes, no } => {
                let pair = (*test_letter, *requirement_letter);
                assert!(
                    pair == ('o', 'l') || pair == ('l', 'o'),
                    "expected letters o/l in some order, got {pair:?}"
                );
                assert!(matches!(**yes, Node::Repeat(_, _)));
                assert!(matches!(**no, Node::Repeat(_, _)));
            }
            other => panic!("expected SoftDoubleLetterSplit root, got {other:?}"),
        }
    }

    #[test]
    fn soft_mirror_first_last_split_works() {
        // Front test, back requirement mirror keeps the miss soft
        let data = words(&["axe", "exa"]);
        let sol = minimal_trees_limited(&data, false, true, Some(1));
        assert_eq!(
            sol.cost,
            Cost { nos: 1, hard_nos: 0, sum_nos: 1, sum_hard_nos: 0, depth: 1, word_count: 2 }
        );
        match &sol.trees[0] {
            Node::SoftMirrorPosSplit { test_letter, test_index, test_from_end, requirement_index, requirement_from_end, .. } => {
                assert_eq!((*test_letter, *test_index, *test_from_end, *requirement_index, *requirement_from_end), ('a', 1, false, 1, true));
            }
            other => panic!("expected SoftMirrorPosSplit root, got {other:?}"),
        }
    }
}
