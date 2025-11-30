use serde::Serialize;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

use crate::node::Solution;
use crate::api::minimal_trees;
use crate::merged::MergedNode;

#[derive(Serialize)]
struct WasmCostSummary {
    max_hard_nos: u32,
    max_nos: u32,
    sum_hard_nos: u32,
    sum_nos: u32,
    word_count: u32,
    avg_hard_nos: f32,
    avg_nos: f32,
}

#[derive(Serialize)]
struct WasmSolution {
    cost: WasmCostSummary,
    merged_tree: MergedNode,
}

fn words_from_js(value: JsValue) -> Result<Vec<String>, JsValue> {
    from_value(value)
        .map_err(|e| JsValue::from_str(&format!("Words must be an array of strings: {e}")))
}

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

    // Merge all optimal trees into a single navigable structure
    let merged_tree = MergedNode::merge(&sol.trees);

    WasmSolution {
        cost: WasmCostSummary {
            max_hard_nos: sol.cost.hard_nos,
            max_nos: sol.cost.nos,
            sum_hard_nos: sol.cost.sum_hard_nos,
            sum_nos: sol.cost.sum_nos,
            word_count,
            avg_hard_nos,
            avg_nos,
        },
        merged_tree,
    }
}

/// WebAssembly entry point: solve for the provided words and return all optimal trees.
#[wasm_bindgen]
pub fn solve_words(
    words: JsValue,
    allow_repeat: bool,
    prioritize_soft_no: bool,
) -> Result<JsValue, JsValue> {
    let words_vec = words_from_js(words)?;
    if words_vec.is_empty() {
        return Err(JsValue::from_str("Please supply at least one word."));
    }
    if words_vec.len() > 32 {
        return Err(JsValue::from_str("Solver supports up to 32 words."));
    }

    let sol = minimal_trees(&words_vec, allow_repeat, prioritize_soft_no);
    to_value(&summary_from_solution(&sol))
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}")))
}

/// Convenience helper exposed to JS: return the Zodiac word list.
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
