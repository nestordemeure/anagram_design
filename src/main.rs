use anagram_design::{format_tree, minimal_trees_limited};

fn zodiac_words() -> Vec<String> {
    vec![
        "aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

fn print_solutions(allow_repeat: bool) {
    let words = zodiac_words();
    const DISPLAY: usize = 10;
    let result = minimal_trees_limited(&words, allow_repeat, Some(DISPLAY));
    let preview = DISPLAY.min(result.trees.len());
    println!(
        "Allow repeat: {} | Best cost = (nos {}, repeat {}, depth {}) | {} tree(s)",
        allow_repeat,
        result.cost.nos,
        result.cost.repeats,
        result.cost.depth,
        result.trees.len()
    );
    for (idx, tree) in result.trees.iter().take(preview).enumerate() {
        println!("--- Tree {} ---\n{}", idx + 1, format_tree(tree));
    }
    if result.trees.len() > preview {
        let more = result.trees.len() - preview;
        if result.exhausted {
            println!("... {} stored (limit reached, more optimal trees exist)", more);
        } else {
            println!("... {} more optimal tree(s) omitted", more);
        }
    } else if result.exhausted {
        println!("(Result list truncated; additional optimal trees exist)");
    }
}

fn main() {
    print_solutions(true);
    println!();
    print_solutions(false);
}
