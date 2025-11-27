# Annagram Design

Minimal-cost “annagram” trees for a set of words, implemented in Rust.

## Model

Nodes are yes/no questions that partition the word set:
- **Hard splits**
  - `Contains 'x'?`
  - `First letter 'x'?`
  - `Last letter 'x'?`
  - A No edge adds 1 to both `nos` and `hard_nos`.
- **Soft splits**
  - Contains soft: `Contains 'i'? (all No contain 'e')` for defined pairs (E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E).
  - Positional soft: `First letter 'a'? (all No have 'a' second)` and `Last letter 's'? (all No have 's' second-to-last)`.
  - Double-letter soft: `Double 'o'? (all No double 'l')` — Yes branch has two of the test letter; No branch has two of a different uniform letter.
  - A No edge adds 1 to `nos` only.
- **Leaves / Repeat** (only when exactly two words): cost `(0,0,0,0,0)`.

### Cost (lexicographically minimized)
1. `nos` — max No edges on any root→leaf path (component-wise max across branches).
2. `hard_nos` — max hard No edges on any path.
3. `sum_nos` — weighted sum of No edges (words in the No branch each add 1).
4. `sum_hard_nos` — weighted sum of hard No edges.
5. `depth` — max tree depth.

Only the first 5 optimal trees are stored/displayed; truncation is noted but optimality still holds.

## Running

```bash
cargo run --quiet
```

The binary prints the optimal trees for the Zodiac word set twice, once allowing `Repeat` nodes and once disalowing them.

## Testing

```bash
cargo test
```

Includes regression tests and a Zodiac cost check for both settings.

## TODO

* should hard no be the first criteria in our cost function?

* soft no:
  * introduce sounds and not just letters?
