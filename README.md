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

Some letters have reciprocals, other letters with which they might get confused: E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E, A/R (those relations go both ways: E is the reciprocal of I, and I is the reciprocal of E).
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

Splits (except Leaves and Repeat) have a primary letter and a secondary letter.

**Hard splits**: Primary letter P equals secondary letter (the letter being tested).
**Soft splits**: Primary letter P (for yes branch) differs from secondary letter S (for no branch).

#### Touched Letters

When a split uses primary P and secondary S:
- P is **touched** in both branches (yes and no).
- S is **touched** only in the no branch.

Descendants cannot use touched letters as their primary or secondary, except where noted below.

#### Basic Rule

- **Yes branch**: P is touched; S is untouched (can be used as primary or secondary).
- **No branch**: Both P and S are touched (neither can be used).

#### Exceptions (Immediate Children Only)

There are three split **classes**: Contains → Positional (first/second/third/last/etc.) → Double/Triple.

Immediate children may use touched letters as primary when moving **same-class or downward**:
- After a **Contains 'P'?**, the yes-branch child can use P as primary (for another Contains, any positional, or Double/Triple).
- After a **soft Contains 'P'? (all No contain 'S')**, the no-branch child can use S as primary (same movement rules).
- After a **Positional 'P'**, the yes-branch child can use P as primary if it's another positional (same or different position), Double, or Triple.
- After a **soft Positional 'P'? (no-branch constraint uses 'S')**, the no-branch child can use S as primary if it's another positional, Double, or Triple.
- After a **Double/Triple 'P'**, children can use P as primary if they are also Double or Triple.

These exceptions **chain**: you can do Contains P → First P → Double P, as long as each step moves same-class or downward.

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

Baseline:
* Allow repeat: true | Prioritize soft no: false | Best cost = (max hard no 1, max no 2, avg hard no 0.5, avg no 0.9, depth 6) | 5 tree(s)
* Allow repeat: true | Prioritize soft no: true | Best cost = (max hard no 1, max no 2, avg hard no 0.2, avg no 1.2, depth 6) | 5 tree(s)
* Allow repeat: false | Prioritize soft no: false | Best cost = (max hard no 1, max no 2, avg hard no 0.5, avg no 1.3, depth 6) | 5 tree(s)
* Allow repeat: false | Prioritize soft no: true | Best cost = (max hard no 1, max no 2, avg hard no 0.4, avg no 1.4, depth 5) | 5 tree(s)

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

* further optimizations
  * use perf / flamegraph to explore costs
  * try a dijkstra type of algorithm instead as this problem might be a fit
    * are some elements of the codebase remnents of the previous approach that would be best modified for the new one?
    * update readme / comments

* further subtleties (new soft constraints):
  * introduce sounds-based subtleties
