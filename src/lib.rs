use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    /// Number of No-edges on the heaviest path (primary objective).
    pub nos: u32,
    /// Number of hard No-edges on the heaviest path (secondary objective).
    pub hard_nos: u32,
    /// Sum of No-edges weighted by word count (tertiary objective).
    pub sum_nos: u32,
    /// Sum of hard No-edges weighted by word count (quaternary objective).
    pub sum_hard_nos: u32,
    /// Total depth (edges) on that path (quinary tie-breaker).
    pub depth: u32,
    /// Number of words in this subtree.
    pub word_count: u32,
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.nos.cmp(&other.nos) {
            Ordering::Equal => match self.hard_nos.cmp(&other.hard_nos) {
                Ordering::Equal => match self.sum_nos.cmp(&other.sum_nos) {
                    Ordering::Equal => match self.sum_hard_nos.cmp(&other.sum_hard_nos) {
                        Ordering::Equal => self.depth.cmp(&other.depth),
                        ord => ord,
                    },
                    ord => ord,
                },
                ord => ord,
            },
            ord => ord,
        }
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
    /// Bitmask of forbidden letters (bit for each letter a-z used in ancestor soft nos)
    forbidden_letters: u32,
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
    last_letter_masks: [u16; 26],
    second_to_last_letter_masks: [u16; 26],
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
];

fn mask_count(mask: u16) -> u32 {
    mask.count_ones()
}

fn single_word_from_mask(mask: u16, words: &[String]) -> Option<String> {
    let idx = mask.trailing_zeros() as usize;
    if idx < words.len() {
        Some(words[idx].clone())
    } else {
        None
    }
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
    forbidden_letters: u32,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let key = Key { mask, allow_repeat, forbidden_letters };
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

    for (idx, letter_mask) in ctx.letter_masks.iter().enumerate() {
        let yes = mask & letter_mask;
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !letter_mask;
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_letters, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_letters, limit, memo);

        // Adding this split increases depth on both sides; "nos" and "hard_nos" increment along No.
        // cost = (0,0,1) + max(yes, no + (1,1,0))
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos + 1,
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1; // true tree height
        // Calculate weighted sums: words in no branch encounter 1 additional hard no edge
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
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
            Some(current) => match branch_cost.cmp(&current) {
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

        // Check if either letter in this pair is forbidden
        let test_bit = 1u32 << test_idx;
        let requirement_bit = 1u32 << requirement_idx;
        if forbidden_letters & (test_bit | requirement_bit) != 0 {
            continue; // One or both letters in this pair are forbidden
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

        // Forbid both letters in children (any soft no containing either letter is now forbidden)
        let child_forbidden = forbidden_letters | test_bit | requirement_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, limit, memo);

        // Soft split: nos increments, but hard_nos does not
        // cost = (0,0,1) + max(yes, no + (1,0,0))
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos, // soft no does not increment hard_nos
            sum_nos: no_sol.cost.sum_nos,
            sum_hard_nos: no_sol.cost.sum_hard_nos,
            depth: no_sol.cost.depth,
            word_count: no_sol.cost.word_count,
        };
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        // Calculate weighted sums: words in no branch encounter 1 additional soft no edge
        // (increments sum_nos but not sum_hard_nos)
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos; // no increment for soft no
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
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
            Some(current) => match branch_cost.cmp(&current) {
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

    // First-letter hard splits
    for (idx, letter_mask) in ctx.first_letter_masks.iter().enumerate() {
        let yes = mask & letter_mask;
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !letter_mask;
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_letters, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_letters, limit, memo);

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
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
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
            Some(current) => match branch_cost.cmp(&current) {
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
    for idx in 0..26 {
        // Check if this letter is forbidden
        let letter_bit = 1u32 << idx;
        if forbidden_letters & letter_bit != 0 {
            continue;
        }

        let yes = mask & ctx.first_letter_masks[idx];
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !ctx.first_letter_masks[idx];

        // Check if all items in the "no" set have the same letter as second letter
        if no & ctx.second_letter_masks[idx] != no {
            continue;
        }

        // Forbid this letter in children
        let child_forbidden = forbidden_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, limit, memo);

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
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
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
            Some(current) => match branch_cost.cmp(&current) {
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

    // Last-letter hard splits
    for (idx, letter_mask) in ctx.last_letter_masks.iter().enumerate() {
        let yes = mask & letter_mask;
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !letter_mask;
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_letters, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_letters, limit, memo);

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
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
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
            Some(current) => match branch_cost.cmp(&current) {
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
    for idx in 0..26 {
        // Check if this letter is forbidden
        let letter_bit = 1u32 << idx;
        if forbidden_letters & letter_bit != 0 {
            continue;
        }

        let yes = mask & ctx.last_letter_masks[idx];
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !ctx.last_letter_masks[idx];

        // Check if all items in the "no" set have the same letter as second-to-last letter
        if no & ctx.second_to_last_letter_masks[idx] != no {
            continue;
        }

        // Forbid this letter in children
        let child_forbidden = forbidden_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, limit, memo);

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
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let total_sum_nos = yes_sol.cost.sum_nos + no_sol.cost.sum_nos + no_sol.cost.word_count;
        let total_sum_hard_nos = yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos;
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
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
            Some(current) => match branch_cost.cmp(&current) {
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

pub fn minimal_trees(words: &[String], allow_repeat: bool) -> Solution {
    minimal_trees_limited(words, allow_repeat, None)
}

pub fn minimal_trees_limited(words: &[String], allow_repeat: bool, limit: Option<usize>) -> Solution {
    assert!(words.len() <= 16, "bitmask solver supports up to 16 words");
    let letter_masks = make_letter_masks(words);
    let first_letter_masks = make_first_letter_masks(words);
    let second_letter_masks = make_second_letter_masks(words);
    let last_letter_masks = make_last_letter_masks(words);
    let second_to_last_letter_masks = make_second_to_last_letter_masks(words);
    let ctx = Context {
        words,
        letter_masks,
        first_letter_masks,
        second_letter_masks,
        last_letter_masks,
        second_to_last_letter_masks,
    };
    let mask = if words.len() == 16 { u16::MAX } else { (1u16 << words.len()) - 1 };
    let mut memo = HashMap::new();
    solve(mask, &ctx, allow_repeat, 0, limit, &mut memo)
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
    fn repeat_beats_depth_for_two_words() {
        let data = words(&["alpha", "beta"]);
        let with_repeat = minimal_trees(&data, true);
        let without_repeat = minimal_trees(&data, false);
        assert!(with_repeat.cost < without_repeat.cost);
        assert!(matches!(with_repeat.trees[0], Node::Repeat(_, _)));
    }

    #[test]
    fn simple_split_cost() {
        let data = words(&["ab", "ac", "b"]);
        let sol = minimal_trees(&data, false);
        // With first/last letter splits, we can now use soft splits more effectively
        // The new optimal tree uses a soft split, reducing sum_hard_nos from 2 to 1
        assert_eq!(sol.cost, Cost { nos: 1, hard_nos: 1, sum_nos: 2, sum_hard_nos: 1, depth: 2, word_count: 3 });
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, Some(1));
        // With soft no pairs, first/last letter splits (same letter in adjacent positions), and the new cost function
        // The same-letter positional soft splits reduce sum_nos by finding efficient adjacency patterns
        assert_eq!(allow_repeat.cost, Cost { nos: 2, hard_nos: 1, sum_nos: 11, sum_hard_nos: 9, depth: 5, word_count: 12 });
        assert_eq!(no_repeat.cost, Cost { nos: 2, hard_nos: 1, sum_nos: 16, sum_hard_nos: 9, depth: 6, word_count: 12 });
    }
}
