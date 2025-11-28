# Anagram Design

Minimal-cost “anagram” trees for a set of words, implemented in Rust.

## Code Structure

The codebase is organized into focused modules:
- **cost.rs** — Cost struct and comparison logic
- **node.rs** — Node enum variants and combinators
- **constraints.rs** — Letter constraint rules and soft-no pairs
- **context.rs** — Word masks and partition iterators
- **solver.rs** — Core recursive solver algorithm
- **format.rs** — ASCII tree rendering
- **api.rs** — Public API (`minimal_trees`, `minimal_trees_limited`)
- **wasm.rs** — WebAssembly bindings (conditional)

## Model

Nodes are yes/no questions that partition the word set:
- **Hard splits**
  - `Contains 'x'?`
  - `First letter 'x'?`
  - `Last letter 'x'?`
  - A No edge adds 1 to both `nos` and `hard_nos`.
- **Soft splits**
  - Contains soft: `Contains 'i'? (all No contain 'e')` for defined pairs (E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E, A/R).
  - Positional soft: `First letter 'a'? (all No have 'a' second)` and `Last letter 's'? (all No have 's' second-to-last)`.
  - Positional mirror soft: mirror the same letter between start/end positions: `First letter 'a'? (all No have 'a' last)`, `Second letter 'a'? (all No have 'a' second-to-last)`, `Third letter 'a'? (all No have 'a' third-to-last)`, plus the reverse direction (last→first, etc.).
  - Double-letter soft: `Double 'o'? (all No double 'l')` — Yes branch has two of the test letter; No branch has two of a different uniform letter.
  - A No edge adds 1 to `nos` only.
- **Leaves / Repeat**: at any point you can "name" a specific word; Yes resolves it, No continues with the rest (with repeat disabled below). Adds 0 `nos`/`hard_nos` and 1 `depth`.

### Splits (rewamped and systematized)

Some letters have reciprocals, other letters with which they migh get confused: E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E, A/R (those relations go both ways: E is the reciprocal of I, and I is the reciprocal of E).
In this section we will call `A` a random letter, `A-` its reciprocal, and `B` any other random letter.

Splits (with the exception of Leaves and Repeat) all have hard baseline, and soft variants (note the use of reciprocal to create soft variant, as well as the use of nearby position, and mirror positions):
* `Contains 'A'?`
  * `(all No contain 'A-')`
* `First letter 'A'?`
  * `(all No have 'A-' first)`
  * `(all No have 'A' second)`
  * `(all No have 'A' last)`
* `Second letter 'A'?`
  * `(all No have 'A' first)`
  * `(all No have 'A-' second)`
  * `(all No have 'A' third)`
  * `(all No have 'A' second-to-last)`
* `Third letter 'A'?`
  * `(all No have 'A' second)`
  * `(all No have 'A-' third)`
  * `(all No have 'A' third-to-last)`
* `Third-to-last letter 'A'?`
  * `(all No have 'A' third)`
  * `(all No have 'A-' third-to-last)`
  * `(all No have 'A' second-to-last)`
* `Second-to-last letter 'A'?`
  * `(all No have 'A' second)`
  * `(all No have 'A' third-to-last)`
  * `(all No have 'A-' second-to-last)`
  * `(all No have 'A' last)`
* `Last letter 'A'?`
  * `(all No have 'A' first)`
  * `(all No have 'A' second-to-last)`
  * `(all No have 'A-' last)`
* `Double 'A'?`
  * `(all No have double 'B')`
* `Triple 'A'?`
  * `(all No have triple 'B')`

### Constraints

Splits (with the exeption of Leaves and Repeat) come with a primary letter and a secondary letter.

Hard splits have only a primary letter, it is the letter being tested upon.
We can consider that letter is both their primary and secondary letter, to simplify things.

Soft split have a primary letter, used for the `yes` branch, and a secondary letter, used as a backup for the `no` branch.

Given a split of primary letter P and secondary letter S:
* later splits in its `yes` branch *cannot* have P as their primary or secondary letter (they *can* use S in the `yes` branch),
* later splits in its `no` branch *cannot* have P, *nor S*, as their primary or secondary letter.

That rule has one exeption:
* a hard contain, of primary letter P, *can* be directly followed by a split whose primary letter is P in its `yes` branch
* a soft contain, of primary letter P and secondary letter S, *can* be directly followed by a split whose primary letter is P in its `yes` branch, and *can* be directly followed by a split whose primary letter is S in its `no` branch

There is already some (imperfect and incomplete) logic around those rules in the code, but it needs to be unified to both simplify it and ensure correctness and exhaustivity across operations.

### Cost (lexicographically minimized)

1. `hard_nos` — max hard No edges on any root→leaf path (component-wise max across branches).
2. `nos` — max No edges on any path.
3. `sum_hard_nos` — weighted sum of hard No edges.
4. `sum_nos` — weighted sum of No edges (words in the No branch each add 1).
5. `depth` — max tree depth.

Only the first 5 optimal trees are stored/displayed; truncation is noted but optimality still holds.

## Running

```bash
cargo run --quiet
```

The binary prints the optimal trees for the Zodiac word set twice, once allowing `Repeat` nodes and once disallowing them.

## Testing

```bash
cargo test
```

Includes regression tests and a Zodiac cost check for both settings.

## Web demo (WASM)

Build and view the browser UI (uses Pico CSS and the wasm-bindgen JS shim):

```bash
# Setup toolchain, once
rustup target add wasm32-unknown-unknown

# Build the code to serve it
cargo build --lib --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir docs/pkg --no-typescript target/wasm32-unknown-unknown/release/anagram_design.wasm

# Test the webpage
python3 -m http.server 8000 --directory docs             # then open http://localhost:8000
```

Use `wasm-bindgen-cli 0.2.95` to match the pinned crate version (or update both in lockstep).

To publish on GitHub Pages, point Pages at the `docs/` directory so the bundled `pkg/` assets are served alongside `index.html`.

## TODO

* make hard soft constraints more printipled: all soft constraints have a hard equivalent, some hard ones have several soft ones
* add another exption to the "do not use the letter" rule (around first actually second)

* further subtleties (mew soft constraints):
  * introduce sounds-based subtleties
