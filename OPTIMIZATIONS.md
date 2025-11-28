# Optimization Notes

## Evaluation Process

### Baseline (before any optimizations)
- Build: `cargo build --release --quiet`
- Timing command (stdout discarded): `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`
- Runs (6x after one release build): 12.81s, 12.71s, 13.49s, 13.75s, 13.89s, 13.93s  
- Baseline metric: **min runtime = 12.71s**
- Machine constraints: none specified; same method to be reused for comparisons.

### Philosophy
- Always benchmark on top of the current fastest code; the baseline evolves as improvements land.  
- Each experiment: build release once, run 6 timed executions, take the minimum; run tests.  
- Record when an idea is discarded to avoid repeating unhelpful paths.

### per lead
1) Create a branch `optim/<lead-name>`.  
2) Implement the change.  
3) `cargo build --release --quiet` once.  
4) Run the timing command 6x; record individual times and the minimum.  
5) Log results and observations here.  
6) Ask before starting any new lead.

## Attempts

### Cheaper hashing for memo table
- Branch: `optim-fast-hash` (could not use `optim/...` because branch `optim` already exists).  
- Change: swapped std `HashMap` for `hashbrown::HashMap` (AHash) in the solver memo.  
- Build: `cargo build --release --quiet`  
- Timing command: `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`  
- Runs (6x): 12.65s, 13.20s, 13.31s, 12.99s, 13.29s, 13.16s  
- Result: **min runtime = 12.65s** (baseline 12.71s) → negligible improvement (~0.06s).  
- Notes: Effect within run-to-run noise; minimal benefit for this workload. Will stick to the default std hasher for simplicity going forward.

### Partition reuse (avoid Vec allocations)
- Branch: `optim-partition-reuse`.  
- Change: replace `partitions()` allocation with an iterator (`Partitions`) to stream partitions without heap allocation.  
- Build: `cargo build --release --quiet`  
- Timing command: `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`  
- Runs (6x): 12.16s, 12.63s, 12.73s, 13.00s, 12.80s, 12.89s  
- Result: **min runtime = 12.16s** (baseline 12.71s) → ~0.55s improvement (~4.3%).  
- Notes: Small but consistent gain; zero-allocation partition iteration seems worthwhile.

### Reduce tree cloning (Rc children)
- Branch: `optim-reduce-cloning`.  
- Change: store child pointers as `Rc<Node>` and return `Vec<Rc<Node>>` from the solver to share subtrees and avoid deep cloning when enumerating optimal trees.  
- Build: `cargo build --release --quiet`  
- Tests: `cargo test --quiet` (all pass).  
- Timing command: `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`  
- Runs (6x): 2.45s, 2.51s, 2.43s, 2.42s, 2.37s, 2.46s  
- Result: **min runtime = 2.37s** (baseline 12.71s) → ~10.34s faster (~81%).  
- Notes: Large speedup from eliminating repeated full-tree cloning; keeps public API behavior the same (formatting still works).

### Combined Rc + partition iterator
- Branch: `optim-combined` (merged `optim-reduce-cloning` + `optim-partition-reuse`).  
- Build: `cargo build --release --quiet`  
- Tests: `cargo test --quiet` (all pass).  
- Timing command: `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`  
- Runs (6x): 2.36s, 2.25s, 2.31s, 2.24s, 2.30s, 2.32s  
- Result: **min runtime = 2.24s** (baseline at that time).  
- Notes: Platform for subsequent experiments (superseded by later baselines).

### Cache letters_present per mask (discarded)
- Branch: `optim-letters-present-cache` (discarded).  
- Change: precompute `present_letters` for all masks and reuse in solver key/constraint pruning.  
- Timing runs showed a slowdown vs baseline; not kept.

### SmallVec for best trees buffer
- Branch: `optim-smallvec` (from `optim-combined`).  
- Change: use `SmallVec<[NodeRef; 5]>` for the hot `best_trees` buffer (still spills to heap if more than 5).  
- Build: `cargo build --release --quiet`  
- Tests: `cargo test --quiet` (all pass).  
- Timing command: `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`  
- Runs (6x): 2.20s, 2.21s, 2.17s, 2.18s, 2.16s, 2.16s  
- Result: **min runtime = 2.16s** (new baseline).  
- Notes: ~3.6% faster than 2.24s baseline; safe even if tree cap increases because `SmallVec` spills to heap beyond inline capacity.

### Hashbrown memo (revisited on fast baseline)
- Branch: `optim-fast-hash-again` (from `optim-smallvec`).  
- Change: swap std `HashMap` for `hashbrown::HashMap` in the memo table (fast hash).  
- Build: `cargo build --release --quiet`  
- Tests: `cargo test --quiet` (all pass).  
- Timing command: `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`  
- Runs (6x): 1.89s, 1.93s, 1.86s, 1.85s, 1.88s, 1.91s  
- Result: **min runtime = 1.85s** (baseline was 2.16s) → ~14% faster.  
- Notes: On the optimized base, the faster hasher shows a clear win; worth keeping.
