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
  - Contains soft: `Contains 'i'? (all No contain 'e')` for defined pairs (E/I, C/K, S/Z, I/L, M/N, U/V, O/Q, C/G, B/P, I/T, R/E, A/R).
  - Positional soft: `First letter 'a'? (all No have 'a' second)` and `Last letter 's'? (all No have 's' second-to-last)`.
  - Positional mirror soft: mirror the same letter between start/end positions: `First letter 'a'? (all No have 'a' last)`, `Second letter 'a'? (all No have 'a' second-to-last)`, `Third letter 'a'? (all No have 'a' third-to-last)`, plus the reverse direction (last→first, etc.).
  - Double-letter soft: `Double 'o'? (all No double 'l')` — Yes branch has two of the test letter; No branch has two of a different uniform letter.
  - A No edge adds 1 to `nos` only.
- **Leaves / Repeat** (only when exactly two words): cost `(0,0,0,0,0)`.

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

The binary prints the optimal trees for the Zodiac word set twice, once allowing `Repeat` nodes and once disalowing them.

## Testing

```bash
cargo test
```

Includes regression tests and a Zodiac cost check for both settings.

## Web demo (WASM)

Build and view the browser UI (uses Pico CSS and the wasm-bindgen JS shim):

```bash
rustup target add wasm32-unknown-unknown               # once
cargo build --lib --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir web/pkg --no-typescript target/wasm32-unknown-unknown/release/anagram_design.wasm
python3 -m http.server 8000 --directory web             # then open http://localhost:8000
```

Use `wasm-bindgen-cli 0.2.95` to match the pinned crate version (or update both in lockstep).

UI features:
- Word list textarea (defaults to the Zodiac signs)
- Toggle `Allow repeat` and `Prioritize soft-no`
- Limit the number of stored optimal trees (0 = unlimited)
- ASCII render of each top tree plus cost summary

To publish on GitHub Pages, point Pages at the `web/` directory so the bundled `pkg/` assets are served alongside `index.html`.

## TODO

* soft no:
  * introduce sounds and not just letters?
