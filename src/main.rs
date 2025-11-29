use anagram_design::{format_tree, minimal_trees};

fn zodiac_words() -> Vec<String> {
    vec![
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
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

fn print_solutions(allow_repeat: bool, prioritize_soft_no: bool) {
    let words = zodiac_words();
    let word_count = words.len() as u32;
    const DISPLAY: usize = 5;
    let result = minimal_trees(&words, allow_repeat, prioritize_soft_no);
    let preview = DISPLAY.min(result.trees.len());
    let avg_sum_hard = result.cost.sum_hard_nos as f32 / word_count as f32;
    let avg_sum = result.cost.sum_nos as f32 / word_count as f32;
    println!(
        "Allow repeat: {} | Prioritize soft no: {} | Best cost = (max hard no {}, max no {}, avg hard no {:.1}, avg no {:.1}, depth {}) | {} tree(s)",
        allow_repeat,
        prioritize_soft_no,
        result.cost.hard_nos,
        result.cost.nos,
        avg_sum_hard,
        avg_sum,
        result.cost.depth,
        result.trees.len()
    );
    for (idx, tree) in result.trees.iter().take(preview).enumerate() {
        println!("--- Tree {} ---\n{}", idx + 1, format_tree(tree));
    }
    if result.trees.len() > preview {
        let more = result.trees.len() - preview;
        println!("... {} more optimal tree(s) omitted from display", more);
    }
}

fn main() {
    print_solutions(true, false);
    println!();
    print_solutions(true, true);
    println!();
    print_solutions(false, false);
    println!();
    print_solutions(false, true);
}
