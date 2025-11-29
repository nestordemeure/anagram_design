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

**Reciprocal pairs**: E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E, A/R (bidirectional).

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

**Same-index restriction**: When chaining positional splits with the same letter, the two positions must not refer to the same absolute index in any word. Example: in "Leo" (3 letters), "Second" (index 1) and "Second-to-last" (index 1) refer to the same position, so they cannot be chained.

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

* further subtleties:
  * introduce sounds-based soft splits?

in this:
```
Contains 'R'? (all No contain 'E')
│└─ No: Second letter 'E'? (all No have 'I' second)
│   │└─ No: Pisces
│   │
│   Contains 'L'? (all No contain 'I') ▼
│   │└─ No: Gemini
│   │
│   └─ Leo
```
why do i not get "Second letter 'E'? (all No have 'E' second-to-last)" as an option?
