# Annagram Design

Minimal-cost “annagram” trees for a set of words, implemented in Rust.

## Model

- Each node tests membership of a letter. Words containing the letter go to **Yes**, the rest to **No**.
- Leaf: single word, cost `(0,0)`.
- Split: cost `(1,0) + max(cost(Yes), cost(No))`.
- Repeat node (optional, only when exactly two words): cost `(0,1)`.
- Trees are compared lexicographically by `(depth, repeats)` (lower is better).

## Running

```bash
cargo run --quiet
```

The binary prints the optimal trees for the Zodiac word set twice:
1. **Allow repeat nodes** (best cost: depth 3, repeats 1)
2. **Forbid repeat nodes** (best cost: depth 4, repeats 0)

Only the first 10 trees are shown; the search still computes all optimal trees.

## Testing

```bash
cargo test
```

Includes regression tests and a Zodiac cost check for both settings.
