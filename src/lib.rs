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
    /// Bitmask of forbidden letters (bit for each letter a-z used in ancestor soft nos)
    forbidden_letters: u32,
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
    last_letter_masks: [u16; 26],
    second_to_last_letter_masks: [u16; 26],
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
    forbidden_letters: u32,
    known_letters: u32,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let key = Key { mask, allow_repeat, forbidden_letters, known_letters };
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
        // In the YES branch, we know this letter exists in all words
        let letter_bit = 1u32 << idx;
        let yes_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_letters, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_letters, known_letters, limit, memo);

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

        // Skip if we already know the requirement_letter exists in all words
        if known_letters & requirement_bit != 0 {
            continue; // requirement_letter is already known, soft test is redundant
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
        // In the YES branch, we know the test_letter exists; in the NO branch, we know the requirement_letter exists
        let yes_known = known_letters | test_bit;
        let no_known = known_letters | requirement_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, no_known, limit, memo);

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

    // Soft double-letter splits: Yes has two of test_letter; No has two of a different uniform letter
    for test_idx in 0..26 {
        let test_bit = 1u32 << test_idx;
        if forbidden_letters & test_bit != 0 {
            continue;
        }

        let yes = mask & ctx.double_letter_masks[test_idx];
        if yes == 0 || yes == mask {
            continue; // no partition or everyone has the double letter
        }
        let no = mask & !ctx.double_letter_masks[test_idx];

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
        if forbidden_letters & requirement_bit != 0 {
            continue;
        }

        // Forbid both letters in children
        let child_forbidden = forbidden_letters | test_bit | requirement_bit;
        let yes_known = known_letters | test_bit;
        let no_known = known_letters | requirement_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, no_known, limit, memo);

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
    for (idx, letter_mask) in ctx.first_letter_masks.iter().enumerate() {
        let yes = mask & letter_mask;
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !letter_mask;
        // In the YES branch, we know this letter exists in all words (as first letter)
        let letter_bit = 1u32 << idx;
        let yes_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_letters, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_letters, known_letters, limit, memo);

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

        // Skip if we already know this letter exists in all words
        if known_letters & letter_bit != 0 {
            continue; // letter is already known, soft test is redundant
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
        // In YES branch, we know letter is first; in NO branch, we know letter is second
        // Either way, the letter exists in all words
        let child_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, child_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, child_known, limit, memo);

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
        // In the YES branch, we know this letter exists in all words (as last letter)
        let letter_bit = 1u32 << idx;
        let yes_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_letters, yes_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_letters, known_letters, limit, memo);

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

        // Skip if we already know this letter exists in all words
        if known_letters & letter_bit != 0 {
            continue; // letter is already known, soft test is redundant
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
        // In YES branch, we know letter is last; in NO branch, we know letter is second-to-last
        // Either way, the letter exists in all words
        let child_known = known_letters | letter_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, child_known, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, child_known, limit, memo);

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

pub fn minimal_trees(words: &[String], allow_repeat: bool) -> Solution {
    // Default to keeping at most 5 optimal trees, matching the CLI display cap.
    minimal_trees_limited(words, allow_repeat, Some(5))
}

pub fn minimal_trees_limited(words: &[String], allow_repeat: bool, limit: Option<usize>) -> Solution {
    assert!(words.len() <= 16, "bitmask solver supports up to 16 words");
    let letter_masks = make_letter_masks(words);
    let first_letter_masks = make_first_letter_masks(words);
    let second_letter_masks = make_second_letter_masks(words);
    let last_letter_masks = make_last_letter_masks(words);
    let second_to_last_letter_masks = make_second_to_last_letter_masks(words);
    let double_letter_masks = make_double_letter_masks(words);
    let ctx = Context {
        words,
        letter_masks,
        first_letter_masks,
        second_letter_masks,
        last_letter_masks,
        second_to_last_letter_masks,
        double_letter_masks,
    };
    let mask = if words.len() == 16 { u16::MAX } else { (1u16 << words.len()) - 1 };
    let mut memo = HashMap::new();
    solve(mask, &ctx, allow_repeat, 0, 0, limit, &mut memo)
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
        let with_repeat = minimal_trees(&data, true);
        let without_repeat = minimal_trees(&data, false);
        assert!(with_repeat.cost < without_repeat.cost);
        assert!(matches!(with_repeat.trees[0], Node::Repeat(_, _)));
    }

    #[test]
    fn simple_split_cost() {
        let data = words(&["ab", "ac", "b"]);
        let sol = minimal_trees(&data, false);
        // With the known_letters constraint, we can't use soft tests when the requirement letter is already known
        // This limits optimization slightly compared to before
        assert_eq!(sol.cost, Cost { nos: 1, hard_nos: 1, sum_nos: 2, sum_hard_nos: 2, depth: 2, word_count: 3 });
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, Some(1));
        // Regression test: R/E pair + C/G pair enable all-soft-test trees
        // With R/E and C/G soft pairs, Virgo/Scorpio can be separated by:
        //   1. "Contains 'r'? (all No contain 'e')" - soft
        //   2. "Contains 'c'? (all No contain 'g')" - soft (Scorpio has c, Virgo has g)
        // This achieves hard_nos: 0!
        assert_eq!(allow_repeat.cost, Cost { nos: 2, hard_nos: 1, sum_nos: 11, sum_hard_nos: 7, depth: 5, word_count: 12 });
        assert_eq!(no_repeat.cost, Cost { nos: 2, hard_nos: 1, sum_nos: 16, sum_hard_nos: 7, depth: 6, word_count: 12 });
    }

    #[test]
    fn virgo_scorpio_soft_separation() {
        // Verify that Virgo and Scorpio CAN be separated using only soft tests
        // This is possible with R/E and C/G pairs:
        //   virgo: has {v,i,r,g,o} - has 'r' and 'g', no 'c'
        //   scorpio: has {s,c,o,r,p,i} - has 'r' and 'c', no 'g'
        //   gemini: has {g,e,m,i,n} - has 'e' and 'g', no 'r' or 'c'
        let data = words(&["virgo", "scorpio", "gemini"]);
        let sol = minimal_trees(&data, true);
        // Should achieve hard_nos: 0 using: r/e soft, then c/g soft
        assert_eq!(sol.cost.hard_nos, 0, "Expected 0 hard NOs (all soft), got {}", sol.cost.hard_nos);
    }

    #[test]
    fn recomputed_cost_matches_expected_for_top_tree() {
        // Use the first printed allow_repeat tree to assert its true hard_no count.
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let sol = minimal_trees_limited(&data, true, Some(1));
        let tree = &sol.trees[0];
        let cost = compute_cost(tree);
        assert_eq!(
            cost,
            Cost { nos: 2, hard_nos: 1, sum_nos: 11, sum_hard_nos: 7, depth: 5, word_count: 12 },
            "Recomputed cost for top allow_repeat tree should expose the hard-no count"
        );
    }

    #[test]
    fn solver_advertised_cost_matches_tree_cost_allow_repeat() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let sol = minimal_trees_limited(&data, true, Some(3));
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
        let sol = minimal_trees_limited(&data, true, Some(1));
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
}
