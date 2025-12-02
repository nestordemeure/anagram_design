# Anagram Design

Minimal-cost “anagram” trees for a set of words, implemented in Rust.

## Code Structure

The codebase is organized into focused modules:
- **cost.rs** — Cost struct and comparison logic
- **node.rs** — Node enum variants and combinators
- **constraints.rs** — Letter constraint rules and soft-no pairs
- **context.rs** — Word masks and partition iterators
- **dijkstra_solver.rs** — Cost-guided recursive solver with memoization
- **format.rs** — ASCII tree rendering
- **api.rs** — Public API
- **wasm.rs** — WebAssembly

## Theory

Nodes are yes/no questions that partition the word set. Each split has a **hard** baseline (primary = secondary letter) and **soft** variants (primary ≠ secondary).

### Splits

**Reciprocal pairs**: E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E, A/R, I/J, V/W, Q/G, E/B, E/F, R/P, R/B, T/F, Y/X, Y/V, O/G, P/F, A/H, D/B, J/L (bidirectional).

In the list below, `A` represents any letter, `A-` its reciprocal, and `B` any other letter.
Soft variants use reciprocals, nearby positions, or mirror positions:

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

### Leaves and Repeat

- **Leaf**: Names a specific word. Yes branch resolves it; No branch continues with remaining words.
- **Repeat**: Like Leaf, but re-enables the same word in descendants (disabled by default after first use).

### Constraints

Splits (except Leaves and Repeat) use a **primary letter P** and **secondary letter S**. When a split uses P and S:
- P is **touched** in both branches (yes and no)
- S is **touched** only in the no branch

Descendants cannot use touched letters as their primary or secondary, with these exceptions:

**Split classes**: Contains → Positional (first/second/third/last/etc.) → Double/Triple.

Immediate children may use touched letters as primary when moving **same-class or downward**:
- After **Contains 'P'?**, the yes-branch child can use P as primary (for another Contains, any positional, or Double/Triple)
- After **soft Contains 'P'? (all No contain 'S')**, the no-branch child can use S as primary (same rules)
- After **Positional 'P'**, the yes-branch child can use P as primary if it's another positional, Double, or Triple, **provided the two positions don't refer to the same absolute index**
- After **soft Positional 'P'?**, the no-branch child can use S as primary (same rules)
- After **Double/Triple 'P'**, children can use P as primary if they are also Double or Triple

These exceptions **chain**: you can do Contains P → First P → Double P, as long as each step moves same-class or downward.

**Same-index restriction**: When generating soft positional splits, positions must not collide in words of the NO branch. Example: "Second E? (all No have E second-to-last)" is invalid if any word in the NO branch has both positions referring to the same index.

**Requirement position constraint**: For soft splits using the same letter at different positions (e.g., "Second E? (all No have E second-to-last)"), both the test position AND requirement position are checked against parent constraints. If parent used E at Second, children cannot use "... (all No have E second)" as a requirement.

### Cost

Each node type contributes to cost metrics:
- **Hard splits** (primary = secondary): No edge adds 1 to both `nos` and `hard_nos`
- **Soft splits** (primary ≠ secondary): No edge adds 1 to `nos` only
- **Leaves/Repeat**: Add 1 to `depth` only

Trees are optimized by lexicographically minimizing:

1. `hard_nos` — max hard No edges on any root→leaf path (component-wise max across branches)
2. `nos` — max No edges on any path
3. `sum_hard_nos` — weighted sum of hard No edges
4. `sum_nos` — weighted sum of No edges (words in the No branch each add 1)
5. `depth` — max tree depth

## Usage

### Running

```bash
cargo run --quiet
```

Prints four trees for the Zodiac word set, with various options allowed or disabled.

### Testing

```bash
cargo test
```

Includes regression tests and a Zodiac cost check for several settings.

### Web Demo (WASM)

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

* the pruning logic is broken, it should use comparisons no its own thing

* redeeming yes
  * introcue redeeming_costs, signed integers, scaled by redeeming hits, take priority to their non redeemed equivalent
  * introduce redeeming splits, they have negative redeemed costs
  * gave solver an exhaustive redeeming split adder