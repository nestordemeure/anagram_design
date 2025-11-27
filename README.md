# Annagram Design

Minimal-cost “annagram” trees for a set of words, implemented in Rust.

## Model

- Each node tests membership of a letter. Words containing the letter go to **Yes**, the rest to **No**.
- Leaf: single word, cost `(nos, hard_nos, depth) = (0, 0, 0)`.
- Repeat node (optional, only when exactly two words): cost `(0, 0, 0)`.
- Hard Split: `cost = (0, 0, 1) + max(cost(Yes), cost(No) + (1, 1, 0))`
  - Tests for a letter (e.g., "contains 'a'?")
  - No edge increments both `nos` and `hard_nos`
- Soft Split: `cost = (0, 0, 1) + max(cost(Yes), cost(No) + (1, 0, 0))`
  - Tests for a letter with a requirement (e.g., "contains 'i'? (all No contain 'e')")
  - Only usable if all items in the No branch satisfy the requirement
  - No edge increments `nos` but not `hard_nos`
  - Currently implemented: I/E (test 'i', require 'e' in No items) and E/I (test 'e', require 'i' in No items)
- Cost components:
  - `nos`: counts all "No" edges (hard and soft) on the heaviest path (primary objective)
  - `hard_nos`: counts only hard "No" edges on the heaviest path (secondary objective)
  - `depth`: total edges (tertiary tie-breaker)
- Costs are compared lexicographically `(nos, hard_nos, depth)`; smaller is better.

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
  * introduces ones having to do with letter positions?
