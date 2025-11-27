use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    pub depth: u32,
    pub repeats: u32,
}

impl Cost {
    fn with_extra_depth(self, extra: u32) -> Self {
        Self {
            depth: self.depth + extra,
            repeats: self.repeats,
        }
    }
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.depth.cmp(&other.depth) {
            Ordering::Equal => self.repeats.cmp(&other.repeats),
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
            cost: Cost { depth: 0, repeats: 0 },
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
            best_cost = Some(Cost { depth: 0, repeats: 1 });
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

        let branch_cost = std::cmp::max(yes_sol.cost, no_sol.cost).with_extra_depth(1);

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
    fn helper(node: &Node, indent: &str, output: &mut String) {
        match node {
            Node::Leaf(word) => {
                output.push_str(indent);
                output.push_str(&format!("Leaf: {}\n", word));
            }
            Node::Repeat(a, b) => {
                output.push_str(indent);
                output.push_str(&format!("Repeat: {} / {}\n", a, b));
            }
            Node::Split { letter, yes, no } => {
                output.push_str(indent);
                output.push_str(&format!("? contains '{}':\n", letter));
                let next = format!("{}  ", indent);
                output.push_str(&format!("{}Y -> ", indent));
                match **yes {
                    Node::Leaf(_) | Node::Repeat(_, _) => helper(yes, &format!("{}     ", indent), output),
                    Node::Split { .. } => {
                        output.push('\n');
                        helper(yes, &next, output)
                    }
                }
                output.push_str(&format!("{}N -> ", indent));
                match **no {
                    Node::Leaf(_) | Node::Repeat(_, _) => helper(no, &format!("{}     ", indent), output),
                    Node::Split { .. } => {
                        output.push('\n');
                        helper(no, &next, output)
                    }
                }
            }
        }
    }
    let mut out = String::new();
    helper(node, "", &mut out);
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
        assert_eq!(sol.cost, Cost { depth: 2, repeats: 0 });
    }

    #[test]
    fn zodiac_costs() {
        let data = words(&[
            "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
        ]);
        let allow_repeat = minimal_trees_limited(&data, true, Some(1));
        let no_repeat = minimal_trees_limited(&data, false, Some(1));
        assert_eq!(allow_repeat.cost, Cost { depth: 3, repeats: 1 });
        assert_eq!(no_repeat.cost, Cost { depth: 4, repeats: 0 });
    }
}
