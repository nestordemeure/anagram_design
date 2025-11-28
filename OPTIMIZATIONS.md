# Optimization Notes

Date: 2025-11-28  
Current commit: 9465242

## Baseline (before any optimizations)
- Build: `cargo build --release --quiet`
- Timing command (stdout discarded): `/usr/bin/time -f "%e" cargo run --release --quiet >/dev/null`
- Runs (6x after one release build): 12.81s, 12.71s, 13.49s, 13.75s, 13.89s, 13.93s  
- Baseline metric: **min runtime = 12.71s**
- Machine constraints: none specified; same method to be reused for comparisons.

## Evaluation Process (per lead)
1) Create a branch `optim/<lead-name>`.  
2) Implement the change.  
3) `cargo build --release --quiet` once.  
4) Run the timing command 6x; record individual times and the minimum.  
5) Log results and observations here.  
6) Ask before starting any new lead.

## Optimization Leads to Explore
1) Cheaper hashing for the memo table (e.g., `hashbrown::HashMap` + fast hasher).  
2) Reuse partition results to avoid per-call `Vec` allocations.  
3) Reduce tree cloning in solver (share nodes / defer materialization).  
4) Cache `letters_present` per mask or thread it down to cut repeated scans.

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
