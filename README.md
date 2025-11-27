# Annagram Design

Minimal-cost “annagram” trees for a set of words, implemented in Rust.

## Model

- Nodes test letter properties: **contains** (e.g., "contains 'a'?"), **first letter** (e.g., "first letter 'a'?"), or **last letter** (e.g., "last letter 's'?"). Yes/No branches partition the word set.
- Leaf: single word, cost `(0, 0, 0, 0, 0)`.
- Repeat node (optional, only when exactly two words): cost `(0, 0, 0, 0, 0)`.
- Hard Split: No edge increments both `nos` and `hard_nos`
- Soft Split: No edge increments `nos` but not `hard_nos`
  - Contains soft: "contains 'i'? (all No contain 'e')" — pairs like I/E, C/K, S/Z
  - Positional soft: "first letter 'a'? (all No have 'a' second)" or "last letter 's'? (all No have 's' second-to-last)" — same letter in adjacent positions
- Cost components (lexicographic comparison, smaller is better):
  1. `nos`: max "No" edges on any root-to-leaf path
  2. `hard_nos`: max hard "No" edges on any root-to-leaf path
  3. `sum_nos`: sum of "No" edges weighted by word count
  4. `sum_hard_nos`: sum of hard "No" edges weighted by word count
  5. `depth`: max tree depth

## Running

```bash
cargo run --quiet
```

The binary prints the optimal trees for the Zodiac word set twice, once allowing `Repeat` nodes and once disalowing them.

Only up to 10 optimal trees are stored and displayed; if more exist, the output notes that it was truncated (cost optimality is still certified).

## Testing

```bash
cargo test
```

Includes regression tests and a Zodiac cost check for both settings.

## TODO

* soft no:
  * introduce sounds and not just letters?
  * a given double letter (with all nos ahving a the same, different, double letter in common)
