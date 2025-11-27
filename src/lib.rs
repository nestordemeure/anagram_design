use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    /// Number of No-edges on the heaviest path (primary objective).
    pub nos: u32,
    /// Number of hard No-edges on the heaviest path (secondary objective).
    pub hard_nos: u32,
    /// Total depth (edges) on that path (tertiary tie-breaker).
    pub depth: u32,
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.nos.cmp(&other.nos) {
            Ordering::Equal => match self.hard_nos.cmp(&other.hard_nos) {
                Ordering::Equal => self.depth.cmp(&other.depth),
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
    /// Bitmask of forbidden soft no pairs (each pair gets one bit)
    forbidden_soft_nos: u8,
}

/// Defines a soft no pair: (test_letter, requirement_letter)
/// E/I means: test for 'e', require all No items contain 'i'
/// Each pair implies its reciprocal
#[derive(Debug, Clone, Copy)]
struct SoftNoPair {
    /// First direction: test this letter, require the other in No items
    test_letter: char,
    requirement_letter: char,
    /// Index in the forbidden bitmask (same for both directions)
    pair_index: u8,
}

struct Context<'a> {
    words: &'a [String],
    letter_masks: [u16; 26],
}

/// Define the available soft no pairs
/// Each pair implies both directions, and children cannot use the reciprocal
const SOFT_NO_PAIRS: &[SoftNoPair] = &[
    // E/I pair (pair_index = 0, uses bit 0 in forbidden bitmask)
    SoftNoPair { test_letter: 'e', requirement_letter: 'i', pair_index: 0 },
    SoftNoPair { test_letter: 'i', requirement_letter: 'e', pair_index: 0 },
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
    forbidden_soft_nos: u8,
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let key = Key { mask, allow_repeat, forbidden_soft_nos };
    if let Some(hit) = memo.get(&key) {
        return hit.clone();
    }

    let count = mask_count(mask);

    // Leaf node
    if count == 1 {
        let word = single_word_from_mask(mask, ctx.words).expect("mask must map to a word");
        let sol = Solution {
            cost: Cost { nos: 0, hard_nos: 0, depth: 0 },
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
            best_cost = Some(Cost { nos: 0, hard_nos: 0, depth: 0 });
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
        let yes_sol = solve(yes, ctx, allow_repeat, forbidden_soft_nos, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, forbidden_soft_nos, limit, memo);

        // Adding this split increases depth on both sides; "nos" and "hard_nos" increment along No.
        // cost = (0,0,1) + max(yes, no + (1,1,0))
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos + 1,
            depth: no_sol.cost.depth,
        };
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1; // true tree height
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
            depth: branch_depth,
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
        // Check if this pair is forbidden
        let pair_bit = 1u8 << pair.pair_index;
        if forbidden_soft_nos & pair_bit != 0 {
            continue; // This pair is forbidden
        }

        let test_idx = (pair.test_letter as u8 - b'a') as usize;
        let requirement_idx = (pair.requirement_letter as u8 - b'a') as usize;

        let yes = mask & ctx.letter_masks[test_idx];
        if yes == 0 || yes == mask {
            continue; // does not partition the set
        }
        let no = mask & !ctx.letter_masks[test_idx];

        // Check if all items in the "no" set contain the requirement letter
        if no & ctx.letter_masks[requirement_idx] != no {
            continue; // not all No items contain the requirement letter
        }

        // Forbid this pair (both directions) in children
        let child_forbidden = forbidden_soft_nos | pair_bit;
        let yes_sol = solve(yes, ctx, allow_repeat, child_forbidden, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, child_forbidden, limit, memo);

        // Soft split: nos increments, but hard_nos does not
        // cost = (0,0,1) + max(yes, no + (1,0,0))
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            hard_nos: no_sol.cost.hard_nos, // soft no does not increment hard_nos
            depth: no_sol.cost.depth,
        };
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1;
        let branch_cost = Cost {
            nos: dominant.nos,
            hard_nos: dominant.hard_nos,
            depth: branch_depth,
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

pub fn minimal_trees(words: &[String], allow_repeat: bool) -> Solution {
    minimal_trees_limited(words, allow_repeat, None)
}

pub fn minimal_trees_limited(words: &[String], allow_repeat: bool, limit: Option<usize>) -> Solution {
    assert!(words.len() <= 16, "bitmask solver supports up to 16 words");
    let letter_masks = make_letter_masks(words);
    let ctx = Context { words, letter_masks };
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
        assert_eq!(sol.cost, Cost { nos: 1, hard_nos: 1, depth: 2 });
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, Some(1));
        // With soft no nodes (I/E, E/I) and reciprocal prevention, we achieve (2, 1, 5)
        assert_eq!(allow_repeat.cost, Cost { nos: 2, hard_nos: 1, depth: 5 });
        assert_eq!(no_repeat.cost, Cost { nos: 2, hard_nos: 2, depth: 6 });
    }
}
