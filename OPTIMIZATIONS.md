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
