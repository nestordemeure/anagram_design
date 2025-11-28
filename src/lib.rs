use std::cmp::Ordering;
use std::rc::Rc;
use hashbrown::HashMap;
use smallvec::SmallVec;

#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use serde_wasm_bindgen::{from_value, to_value};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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
    /// Ask directly for a specific word; Yes resolves that word, No continues with the rest.
    Repeat {
        word: String,
        no: Rc<Node>,
    },
    Split {
        letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftSplit {
        /// Letter to test for (e.g., 'i' in I/E)
        test_letter: char,
        /// Letter that all No items must contain (e.g., 'e' in I/E)
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    FirstLetterSplit {
        letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftFirstLetterSplit {
        /// Letter to test as first letter
        test_letter: char,
        /// Letter that all No items must have as second letter
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    LastLetterSplit {
        letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftLastLetterSplit {
        /// Letter to test as last letter
        test_letter: char,
        /// Letter that all No items must have as second-to-last letter
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
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
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftDoubleLetterSplit {
        /// Letter that must appear twice in the Yes branch
        test_letter: char,
        /// Letter (different) that must appear twice in all No items
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
}

type NodeRef = Rc<Node>;
#[derive(Debug, Clone)]
pub struct Solution {
    pub cost: Cost,
    pub trees: Vec<NodeRef>,
    pub exhausted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    mask: u16,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    forbidden_primary: u32,
    forbidden_secondary: u32,
    allowed_primary_once: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Constraints {
    /// Letters forbidden as primary letters in this subtree
    forbidden_primary: u32,
    /// Letters forbidden as secondary letters in this subtree
    forbidden_secondary: u32,
    /// Letters that are temporarily allowed as primary for the *first* split in this subtree
    /// (used for the contain exceptions)
    allowed_primary_once: u32,
}

impl Constraints {
    fn empty() -> Self {
        Constraints {
            forbidden_primary: 0,
            forbidden_secondary: 0,
            allowed_primary_once: 0,
        }
    }

    fn primary_allowed(&self, idx: usize) -> bool {
        let bit = 1u32 << idx;
        (self.forbidden_primary & bit == 0) || (self.allowed_primary_once & bit != 0)
    }

    fn secondary_allowed(&self, idx: usize) -> bool {
        let bit = 1u32 << idx;
        self.forbidden_secondary & bit == 0
    }

    /// Clear one-time allowances when descending; persistent forbiddances stay.
    fn next_level(&self) -> Self {
        Constraints {
            forbidden_primary: self.forbidden_primary,
            forbidden_secondary: self.forbidden_secondary,
            allowed_primary_once: 0,
        }
    }

    fn prune(self, present_letters: u32) -> Self {
        Constraints {
            forbidden_primary: self.forbidden_primary & present_letters,
            forbidden_secondary: self.forbidden_secondary & present_letters,
            allowed_primary_once: self.allowed_primary_once & present_letters,
        }
    }
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
    SoftNoPair {
        test_letter: 'e',
        requirement_letter: 'i',
    },
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 'e',
    },
    // C/K pair - identical hard sound
    SoftNoPair {
        test_letter: 'c',
        requirement_letter: 'k',
    },
    SoftNoPair {
        test_letter: 'k',
        requirement_letter: 'c',
    },
    // S/Z pair - similar sibilants
    SoftNoPair {
        test_letter: 's',
        requirement_letter: 'z',
    },
    SoftNoPair {
        test_letter: 'z',
        requirement_letter: 's',
    },
    // I/L pair - visually similar
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 'l',
    },
    SoftNoPair {
        test_letter: 'l',
        requirement_letter: 'i',
    },
    // M/N pair - nasals
    SoftNoPair {
        test_letter: 'm',
        requirement_letter: 'n',
    },
    SoftNoPair {
        test_letter: 'n',
        requirement_letter: 'm',
    },
    // U/V pair - visually similar
    SoftNoPair {
        test_letter: 'u',
        requirement_letter: 'v',
    },
    SoftNoPair {
        test_letter: 'v',
        requirement_letter: 'u',
    },
    // O/Q pair - visually similar
    SoftNoPair {
        test_letter: 'o',
        requirement_letter: 'q',
    },
    SoftNoPair {
        test_letter: 'q',
        requirement_letter: 'o',
    },
    // C/G pair - visually similar
    SoftNoPair {
        test_letter: 'c',
        requirement_letter: 'g',
    },
    SoftNoPair {
        test_letter: 'g',
        requirement_letter: 'c',
    },
    // B/P pair - voiced/unvoiced
    SoftNoPair {
        test_letter: 'b',
        requirement_letter: 'p',
    },
    SoftNoPair {
        test_letter: 'p',
        requirement_letter: 'b',
    },
    // I/T pair - visually similar
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 't',
    },
    SoftNoPair {
        test_letter: 't',
        requirement_letter: 'i',
    },
    // R/E pair
    SoftNoPair {
        test_letter: 'r',
        requirement_letter: 'e',
    },
    SoftNoPair {
        test_letter: 'e',
        requirement_letter: 'r',
    },
    // A/R pair - similar open shapes in block capitals
    SoftNoPair {
        test_letter: 'a',
        requirement_letter: 'r',
    },
    SoftNoPair {
        test_letter: 'r',
        requirement_letter: 'a',
    },
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
struct Partitions<'a> {
    masks: &'a [u16; 26],
    mask: u16,
    idx: usize,
}

impl<'a> Iterator for Partitions<'a> {
    type Item = (usize, u16, u16);

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < 26 {
            let current_idx = self.idx;
            self.idx += 1;
            let letter_mask = self.masks[current_idx];
            let yes = self.mask & letter_mask;
            if yes == 0 || yes == self.mask {
                continue;
            }
            let no = self.mask & !letter_mask;
            return Some((current_idx, yes, no));
        }
        None
    }
}

fn partitions(mask: u16, masks: &[u16; 26]) -> Partitions<'_> {
    Partitions {
        masks,
        mask,
        idx: 0,
    }
}

fn split_allowed(constraints: &Constraints, primary_idx: usize, secondary_idx: usize) -> bool {
    constraints.primary_allowed(primary_idx) && constraints.secondary_allowed(secondary_idx)
}

fn branch_constraints(
    constraints: &Constraints,
    primary_idx: usize,
    secondary_idx: usize,
    yes_primary_allow: Option<u32>,
    no_primary_allow: Option<u32>,
) -> (Constraints, Constraints) {
    let mut yes = constraints.next_level();
    let mut no = constraints.next_level();

    let primary_bit = 1u32 << primary_idx;
    let secondary_bit = 1u32 << secondary_idx;

    // Apply the general rule
    yes.forbidden_primary |= primary_bit;
    yes.forbidden_secondary |= primary_bit;

    no.forbidden_primary |= primary_bit | secondary_bit;
    no.forbidden_secondary |= primary_bit | secondary_bit;

    // Exception allowances (single-use)
    if let Some(bit) = yes_primary_allow {
        yes.allowed_primary_once |= bit;
    }
    if let Some(bit) = no_primary_allow {
        no.allowed_primary_once |= bit;
    }

    (yes, no)
}

fn letters_present(mask: u16, ctx: &Context<'_>) -> u32 {
    let mut present: u32 = 0;
    for idx in 0..26 {
        if mask & ctx.letter_masks[idx] != 0 {
            present |= 1u32 << idx;
        }
    }
    present
}

fn combine_children(letter: char, left: &NodeRef, right: &NodeRef) -> NodeRef {
    Rc::new(Node::Split {
        letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_soft_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_first_letter_children(letter: char, left: &NodeRef, right: &NodeRef) -> NodeRef {
    Rc::new(Node::FirstLetterSplit {
        letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_soft_first_letter_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftFirstLetterSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_last_letter_children(letter: char, left: &NodeRef, right: &NodeRef) -> NodeRef {
    Rc::new(Node::LastLetterSplit {
        letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_soft_last_letter_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftLastLetterSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_soft_mirror_pos_children(
    test_letter: char,
    test_index: u8,
    test_from_end: bool,
    requirement_index: u8,
    requirement_from_end: bool,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftMirrorPosSplit {
        test_letter,
        test_index,
        test_from_end,
        requirement_index,
        requirement_from_end,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

fn combine_soft_double_letter_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftDoubleLetterSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
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

fn solve(
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

    for (idx, yes, no) in partitions(mask, &ctx.letter_masks) {
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        let letter_bit = 1u32 << idx;
        // Hard contain split; allow primary letter reuse in the YES child only.
        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            idx,
            idx,
            Some(letter_bit),
            None,
        );

        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
        let total_sum_hard_nos =
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
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

        if !split_allowed(&constraints, test_idx, requirement_idx) {
            continue;
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

        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            test_idx,
            requirement_idx,
            Some(test_bit),  // soft contain can reuse P in YES
            Some(requirement_bit), // soft contain can reuse S in NO
        );
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
                                combine_soft_children(
                                    pair.test_letter,
                                    pair.requirement_letter,
                                    y,
                                    n,
                                ),
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
                                combine_soft_children(
                                    pair.test_letter,
                                    pair.requirement_letter,
                                    y,
                                    n,
                                ),
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
        if !constraints.primary_allowed(test_idx) {
            continue;
        }

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

        if !split_allowed(&constraints, test_idx, requirement_idx) {
            continue;
        }

        let (yes_constraints, no_constraints) = branch_constraints(
            &constraints,
            test_idx,
            requirement_idx,
            None,
            None,
        );

        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
                            combine_soft_double_letter_children(
                                test_letter,
                                requirement_letter,
                                y,
                                n,
                            ),
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
                                combine_soft_double_letter_children(
                                    test_letter,
                                    requirement_letter,
                                    y,
                                    n,
                                ),
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
                                combine_soft_double_letter_children(
                                    test_letter,
                                    requirement_letter,
                                    y,
                                    n,
                                ),
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
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        let (yes_constraints, no_constraints) = branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
        let total_sum_hard_nos =
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
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
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        // Check if all items in the "no" set have the same letter as second letter
        if no & ctx.second_letter_masks[idx] != no {
            continue;
        }

        let (yes_constraints, no_constraints) =
            branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
            if !split_allowed(&constraints, idx, idx) {
                continue;
            }
            let yes = mask & position_mask(ctx, false, pos, idx);
            if yes == 0 || yes == mask {
                continue;
            }
            let no = mask & !position_mask(ctx, false, pos, idx);

            // All No items must carry the same letter in the mirrored-from-end position
            if no & position_mask(ctx, true, pos, idx) != no {
                continue;
            }

            let (yes_constraints, no_constraints) =
                branch_constraints(&constraints, idx, idx, None, None);
            let yes_sol = solve(
                yes,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                yes_constraints,
                limit,
                memo,
            );
            let no_sol = solve(
                no,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                no_constraints,
                limit,
                memo,
            );

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
                                combine_soft_mirror_pos_children(
                                    letter, pos, false, pos, true, y, n,
                                ),
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
                                    combine_soft_mirror_pos_children(
                                        letter, pos, false, pos, true, y, n,
                                    ),
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
                                    combine_soft_mirror_pos_children(
                                        letter, pos, false, pos, true, y, n,
                                    ),
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
            if !split_allowed(&constraints, idx, idx) {
                continue;
            }
            let yes = mask & position_mask(ctx, true, pos, idx);
            if yes == 0 || yes == mask {
                continue;
            }
            let no = mask & !position_mask(ctx, true, pos, idx);

            // All No items must carry the same letter in the mirrored-from-start position
            if no & position_mask(ctx, false, pos, idx) != no {
                continue;
            }

            let (yes_constraints, no_constraints) =
                branch_constraints(&constraints, idx, idx, None, None);
            let yes_sol = solve(
                yes,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                yes_constraints,
                limit,
                memo,
            );
            let no_sol = solve(
                no,
                ctx,
                allow_repeat,
                prioritize_soft_no,
                no_constraints,
                limit,
                memo,
            );

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
                                combine_soft_mirror_pos_children(
                                    letter, pos, true, pos, false, y, n,
                                ),
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
                                    combine_soft_mirror_pos_children(
                                        letter, pos, true, pos, false, y, n,
                                    ),
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
                                    combine_soft_mirror_pos_children(
                                        letter, pos, true, pos, false, y, n,
                                    ),
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
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        let (yes_constraints, no_constraints) = branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
        let total_sum_hard_nos =
            yes_sol.cost.sum_hard_nos + no_sol.cost.sum_hard_nos + no_sol.cost.word_count;
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
        if !split_allowed(&constraints, idx, idx) {
            continue;
        }

        // Check if all items in the "no" set have the same letter as second-to-last letter
        if no & ctx.second_to_last_letter_masks[idx] != no {
            continue;
        }

        let (yes_constraints, no_constraints) =
            branch_constraints(&constraints, idx, idx, None, None);
        let yes_sol = solve(
            yes,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            yes_constraints,
            limit,
            memo,
        );
        let no_sol = solve(
            no,
            ctx,
            allow_repeat,
            prioritize_soft_no,
            no_constraints,
            limit,
            memo,
        );

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
        trees: best_trees.into_vec(),
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

pub fn minimal_trees_limited(
    words: &[String],
    allow_repeat: bool,
    prioritize_soft_no: bool,
    limit: Option<usize>,
) -> Solution {
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

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct WasmCostSummary {
    max_hard_nos: u32,
    max_nos: u32,
    sum_hard_nos: u32,
    sum_nos: u32,
    depth: u32,
    word_count: u32,
    avg_hard_nos: f32,
    avg_nos: f32,
}

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct WasmSolution {
    cost: WasmCostSummary,
    trees: Vec<String>,
    exhausted: bool,
}

#[cfg(target_arch = "wasm32")]
fn words_from_js(value: JsValue) -> Result<Vec<String>, JsValue> {
    from_value(value)
        .map_err(|e| JsValue::from_str(&format!("Words must be an array of strings: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn summary_from_solution(sol: &Solution) -> WasmSolution {
    let word_count = sol.cost.word_count;
    let avg_hard_nos = if word_count == 0 {
        0.0
    } else {
        sol.cost.sum_hard_nos as f32 / word_count as f32
    };
    let avg_nos = if word_count == 0 {
        0.0
    } else {
        sol.cost.sum_nos as f32 / word_count as f32
    };

    WasmSolution {
        cost: WasmCostSummary {
            max_hard_nos: sol.cost.hard_nos,
            max_nos: sol.cost.nos,
            sum_hard_nos: sol.cost.sum_hard_nos,
            sum_nos: sol.cost.sum_nos,
            depth: sol.cost.depth,
            word_count,
            avg_hard_nos,
            avg_nos,
        },
        trees: sol.trees.iter().map(format_tree).collect(),
        exhausted: sol.exhausted,
    }
}

/// WebAssembly entry point: solve for the provided words and return the top trees.
/// `limit = 0` means "no limit".
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn solve_words(
    words: JsValue,
    allow_repeat: bool,
    prioritize_soft_no: bool,
    limit: u32,
) -> Result<JsValue, JsValue> {
    let words_vec = words_from_js(words)?;
    if words_vec.is_empty() {
        return Err(JsValue::from_str("Please supply at least one word."));
    }
    if words_vec.len() > 16 {
        return Err(JsValue::from_str("Solver supports up to 16 words."));
    }

    let limit = if limit == 0 {
        None
    } else {
        Some(limit as usize)
    };
    let sol = minimal_trees_limited(&words_vec, allow_repeat, prioritize_soft_no, limit);
    to_value(&summary_from_solution(&sol))
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}")))
}

/// Convenience helper exposed to JS: return the Zodiac word list.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn zodiac_words() -> JsValue {
    let words = vec![
        "aries",
        "taurus",
        "gemini",
        "cancer",
        "leo",
        "virgo",
        "libra",
        "scorpio",
        "sagittarius",
        "capricorn",
        "aquarius",
        "pisces",
    ];
    to_value(&words).expect("serialize zodiac words")
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

    // Display helper: show question letters in uppercase for clarity in ASCII trees
    fn display_letter(c: char) -> char {
        c.to_ascii_uppercase()
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
            Node::Repeat { word, no } => {
                out.push_str(prefix);
                out.push_str("└─ No: Repeat ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str("...\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                render_yes_final(&Node::Leaf(word.clone()), &child_prefix, out);
            }
            Node::Split { letter, yes, no } => {
                // No branch that contains another split
                out.push_str(prefix);
                out.push_str("└─ No: Contains '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // The no-branch's children are indented with "│   "
                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                // The yes branch of this nested split uses └─ (it's the final item in this branch)
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // No branch that contains a soft split
                out.push_str(prefix);
                out.push_str("└─ No: Contains '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No contain '");
                out.push(display_letter(*requirement_letter));
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
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftFirstLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // No branch that contains a soft first letter split
                out.push_str(prefix);
                out.push_str("└─ No: First letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second)\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // No branch that contains a last letter split
                out.push_str(prefix);
                out.push_str("└─ No: Last letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftLastLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // No branch that contains a soft last letter split
                out.push_str(prefix);
                out.push_str("└─ No: Last letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second-to-last)\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftDoubleLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("└─ No: Double '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No double '");
                out.push(display_letter(*requirement_letter));
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
            Node::Repeat { word, no } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Repeat ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str("...\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(&Node::Leaf(word.clone()), prefix, out);
            }
            Node::Split { letter, yes, no } => {
                // For a split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // For a soft split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No contain '");
                out.push(display_letter(*requirement_letter));
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
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftFirstLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // For a soft first letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
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
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftLastLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // For a soft last letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second-to-last)\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftDoubleLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Double '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No double '");
                out.push(display_letter(*requirement_letter));
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
            Node::Repeat { word, no } => {
                out.push_str(prefix);
                out.push_str("Repeat ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str("...\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(&Node::Leaf(word.clone()), prefix, is_final, out);
            }
            Node::Split { letter, yes, no } => {
                // Print the question
                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // Print the question for soft split
                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No contain '");
                out.push(display_letter(*requirement_letter));
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
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftFirstLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // Print the question for soft first letter split
                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
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
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftLastLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // Print the question for soft last letter split
                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second-to-last)\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftDoubleLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("Double '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No double '");
                out.push(display_letter(*requirement_letter));
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

    fn leaves(node: &Node) -> Vec<String> {
        fn walk(node: &Node, out: &mut Vec<String>) {
            match node {
                Node::Leaf(w) => out.push(w.clone()),
                Node::Repeat { word, no } => {
                    out.push(word.clone());
                    walk(no, out);
                }
                Node::Split { yes, no, .. }
                | Node::SoftSplit { yes, no, .. }
                | Node::FirstLetterSplit { yes, no, .. }
                | Node::SoftFirstLetterSplit { yes, no, .. }
                | Node::LastLetterSplit { yes, no, .. }
                | Node::SoftLastLetterSplit { yes, no, .. }
                | Node::SoftMirrorPosSplit { yes, no, .. }
                | Node::SoftDoubleLetterSplit { yes, no, .. } => {
                    walk(yes, out);
                    walk(no, out);
                }
            }
        }

        let mut out = Vec::new();
        walk(node, &mut out);
        out
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

        assert_eq!(
            compare_costs(&soft_first, &hard_first, true),
            Ordering::Less
        );
        assert_eq!(
            compare_costs(&soft_first, &hard_first, false),
            Ordering::Greater
        );
    }

    /// Recompute the full Cost of a tree by walking it, independent of the solver.
    fn compute_cost(node: &Node) -> Cost {
        match node {
            Node::Leaf(_) => Cost {
                nos: 0,
                hard_nos: 0,
                sum_nos: 0,
                sum_hard_nos: 0,
                depth: 0,
                word_count: 1,
            },
            Node::Repeat { no, .. } => {
                let yes_cost = Cost {
                    nos: 0,
                    hard_nos: 0,
                    sum_nos: 0,
                    sum_hard_nos: 0,
                    depth: 0,
                    word_count: 1,
                };
                let no_cost = compute_cost(no);
                Cost {
                    nos: yes_cost.nos.max(no_cost.nos),
                    hard_nos: yes_cost.hard_nos.max(no_cost.hard_nos),
                    sum_nos: yes_cost.sum_nos + no_cost.sum_nos,
                    sum_hard_nos: yes_cost.sum_hard_nos + no_cost.sum_hard_nos,
                    depth: yes_cost.depth.max(no_cost.depth) + 1,
                    word_count: yes_cost.word_count + no_cost.word_count,
                }
            }
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
                let sum_hard_nos =
                    yes_cost.sum_hard_nos + no_cost.sum_hard_nos + no_cost.word_count;
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
        assert!(matches!(&*with_repeat.trees[0], Node::Repeat { .. }));
    }

    #[test]
    fn simple_split_cost() {
        let data = words(&["ab", "ac", "b"]);
        let sol = minimal_trees(&data, false, true);
        assert_eq!(
            sol.cost,
            Cost {
                nos: 1,
                hard_nos: 1,
                sum_nos: 2,
                sum_hard_nos: 2,
                depth: 2,
                word_count: 3
            }
        );
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries",
            "taurus",
            "gemini",
            "cancer",
            "leo",
            "virgo",
            "libra",
            "scorpio",
            "sagittarius",
            "capricorn",
            "aquarius",
            "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, true, Some(1));
        // Regression test: R/E pair + C/G pair enable all-soft-test trees
        // With R/E and C/G soft pairs, Virgo/Scorpio can be separated by:
        //   1. "Contains 'r'? (all No contain 'e')" - soft
        //   2. "Contains 'c'? (all No contain 'g')" - soft (Scorpio has c, Virgo has g)
        // This achieves hard_nos: 0!
        assert_eq!(
            allow_repeat.cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 16,
                sum_hard_nos: 4,
                depth: 6,
                word_count: 12
            }
        );
        assert_eq!(
            no_repeat.cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 16,
                sum_hard_nos: 7,
                depth: 6,
                word_count: 12
            }
        );
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
        assert_eq!(
            sol.cost.hard_nos, 0,
            "Expected 0 hard NOs (all soft), got {}",
            sol.cost.hard_nos
        );
    }

    #[test]
    fn soft_known_letter_pruning_regression() {
        // Under the stricter primary/secondary constraints, we cannot reuse the first split's
        // primary letter ('r') as a secondary letter in its Yes branch. That forces a hard edge
        // in the best tree for this tiny dataset.
        let data = words(&["tr", "r", "e"]);
        let sol = minimal_trees(&data, false, true);
        assert_eq!(
            sol.cost,
            Cost {
                hard_nos: 1,
                nos: 1,
                sum_hard_nos: 1,
                sum_nos: 2,
                depth: 2,
                word_count: 3
            },
            "Expected constrained separation with exactly one hard NO; got {:?}",
            sol.cost
        );
    }

    #[test]
    fn recomputed_cost_matches_expected_for_top_tree() {
        // Use the first printed allow_repeat tree to assert its true hard_no count.
        let data = words(&[
            "aries",
            "taurus",
            "gemini",
            "cancer",
            "leo",
            "virgo",
            "libra",
            "scorpio",
            "sagittarius",
            "capricorn",
            "aquarius",
            "pisces",
        ]);
        let sol = minimal_trees_limited(&data, true, true, Some(1));
        let tree = &sol.trees[0];
        let cost = compute_cost(tree);
        assert_eq!(
            cost,
            Cost {
                nos: 2,
                hard_nos: 1,
                sum_nos: 16,
                sum_hard_nos: 4,
                depth: 6,
                word_count: 12
            }
        );
    }

    #[test]
    fn solver_advertised_cost_matches_tree_cost_allow_repeat() {
        let data = words(&[
            "aries",
            "taurus",
            "gemini",
            "cancer",
            "leo",
            "virgo",
            "libra",
            "scorpio",
            "sagittarius",
            "capricorn",
            "aquarius",
            "pisces",
        ]);
        let sol = minimal_trees_limited(&data, true, true, Some(3));
        for (idx, tree) in sol.trees.iter().take(3).enumerate() {
            let tree_cost = compute_cost(tree);
            assert_eq!(
                sol.cost,
                tree_cost,
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
        let sol = minimal_trees_limited(&data, false, true, Some(1));
        assert_eq!(
            sol.cost,
            Cost {
                nos: 1,
                hard_nos: 1,
                sum_nos: 3,
                sum_hard_nos: 2,
                depth: 3,
                word_count: 4
            }
        );
        match &*sol.trees[0] {
            Node::Split { letter: 'l', yes, no } => {
                assert_eq!(leaves(no), vec!["book".to_string()]);
                if let Node::SoftDoubleLetterSplit {
                    test_letter,
                    requirement_letter,
                    yes: yes_branch,
                    no: no_branch,
                } = &**yes
                {
                    let pair = (*test_letter, *requirement_letter);
                    assert!(
                        pair == ('l', 'o') || pair == ('o', 'l'),
                        "expected letters l/o in some order, got {pair:?}"
                    );
                    let mut yes_leaves = leaves(yes_branch);
                    yes_leaves.sort();
                    assert_eq!(yes_leaves, vec!["ball".to_string(), "tall".to_string()]);

                    let mut no_leaves = leaves(no_branch);
                    no_leaves.sort();
                    assert_eq!(no_leaves, vec!["pool".to_string()]);
                } else {
                    panic!("expected SoftDoubleLetterSplit after 'l' split, got {:?}", &**yes);
                }
            }
            other => panic!("expected leading 'l' hard split, got {other:?}"),
        }
    }

    #[test]
    fn soft_mirror_first_last_split_works() {
        // Front test, back requirement mirror keeps the miss soft
        let data = words(&["axe", "exa"]);
        let sol = minimal_trees_limited(&data, false, true, Some(1));
        assert_eq!(
            sol.cost,
            Cost {
                nos: 1,
                hard_nos: 0,
                sum_nos: 1,
                sum_hard_nos: 0,
                depth: 1,
                word_count: 2
            }
        );
        match &*sol.trees[0] {
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                ..
            } => {
                assert_eq!(
                    (
                        *test_letter,
                        *test_index,
                        *test_from_end,
                        *requirement_index,
                        *requirement_from_end
                    ),
                    ('a', 1, false, 1, true)
                );
            }
            other => panic!("expected SoftMirrorPosSplit root, got {other:?}"),
        }
    }
}
