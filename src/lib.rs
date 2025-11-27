use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    /// Number of No-edges on the heaviest path (primary objective).
    pub nos: u32,
    /// Number of Repeat nodes on that path (secondary objective).
    pub repeats: u32,
    /// Total depth (edges) on that path (tertiary tie-breaker).
    pub depth: u32,
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.nos.cmp(&other.nos) {
            Ordering::Equal => match self.repeats.cmp(&other.repeats) {
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
}

struct Context<'a> {
    words: &'a [String],
    letter_masks: [u16; 26],
}

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
    limit: Option<usize>,
    memo: &mut HashMap<Key, Solution>,
) -> Solution {
    let key = Key { mask, allow_repeat };
    if let Some(hit) = memo.get(&key) {
        return hit.clone();
    }

    let count = mask_count(mask);

    // Leaf node
    if count == 1 {
        let word = single_word_from_mask(mask, ctx.words).expect("mask must map to a word");
        let sol = Solution {
            cost: Cost { nos: 0, repeats: 0, depth: 0 },
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
            best_cost = Some(Cost { nos: 0, repeats: 1, depth: 0 });
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
        let yes_sol = solve(yes, ctx, allow_repeat, limit, memo);
        let no_sol = solve(no, ctx, allow_repeat, limit, memo);

        // Adding this split increases depth on both sides; "nos" only increments along No.
        // cost = (0,0,1) + max(yes, no + (1,0,0))
        let yes_cost = yes_sol.cost;
        let no_cost = Cost {
            nos: no_sol.cost.nos + 1,
            repeats: no_sol.cost.repeats,
            depth: no_sol.cost.depth,
        };
        let dominant = std::cmp::max(yes_cost, no_cost);
        let branch_depth = std::cmp::max(yes_sol.cost.depth, no_sol.cost.depth) + 1; // true tree height
        let branch_cost = Cost {
            nos: dominant.nos,
            repeats: dominant.repeats,
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
    solve(mask, &ctx, allow_repeat, limit, &mut memo)
}

pub fn format_tree(node: &Node) -> String {
    // Mimic the `tree` CLI look with box-drawing characters.
    fn render(node: &Node, prefix: &str, is_last: bool, label: &str, out: &mut String) {
        let connector = if prefix.is_empty() {
            "" // root
        } else if is_last {
            "└─ "
        } else {
            "├─ "
        };

        match node {
            Node::Leaf(word) => {
                out.push_str(prefix);
                out.push_str(connector);
                out.push_str(label);
                out.push_str(&format!("Leaf: {}\n", word));
            }
            Node::Repeat(a, b) => {
                out.push_str(prefix);
                out.push_str(connector);
                out.push_str(label);
                out.push_str(&format!("Repeat: {} / {}\n", a, b));
            }
            Node::Split { letter, yes, no } => {
                out.push_str(prefix);
                out.push_str(connector);
                out.push_str(label);
                out.push_str(&format!("Contains '{}'\n", letter));

                let child_prefix = if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}│   ", prefix)
                };

                render(no, &child_prefix, false, "No: ", out);
                render(yes, &child_prefix, true, "Yes: ", out);
            }
        }
    }

    let mut out = String::new();
    render(node, "", true, "", &mut out);
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
        assert_eq!(sol.cost, Cost { nos: 1, repeats: 0, depth: 2 });
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, Some(1));
        assert_eq!(allow_repeat.cost, Cost { nos: 2, repeats: 0, depth: 5 });
        assert_eq!(no_repeat.cost, Cost { nos: 2, repeats: 0, depth: 6 });
    }
}
