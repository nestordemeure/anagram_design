# Annagram Design

Minimal-cost “annagram” trees for a set of words, implemented in Rust.

## Model

- Each node tests membership of a letter. Words containing the letter go to **Yes**, the rest to **No**.
- Leaf: single word, cost `(nos, repeats, depth) = (0,0,0)`.
- Repeat node (optional, only when exactly two words): cost `(0,1,0)`.
- Split: `cost = (0,0,1) + max(cost(Yes), cost(No) + (1,0,0))`
  - `nos`: counts “No” edges on the heaviest path.
  - `repeats`: counts Repeat nodes on that path.
  - `depth`: total edges (ties only).
- Costs are compared lexicographically `(nos, repeats, depth)`; smaller is better.

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

* remove repeats from cost function
* introduce hard no (the basic) vs soft no (all no items share a given letter: E/I, I/E).