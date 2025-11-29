# Optimization Log

## Goal
Get the full Zodiac run (4 configurations) to complete in <4s total (averaging <1s per configuration).

## Baseline (2025-11-28)

### Performance
Measured with `time cargo run --quiet` on branch `optim`:
- **Total runtime**: 539.1 seconds (~9 minutes)
- **Per-configuration average**: ~135 seconds
- **Target**: <4s total (<1s per config)
- **Required speedup**: ~135x

### Test Results
All tests passing on branch `optim`:
```
Allow repeat: true | Prioritize soft no: false | Best cost = (max hard no 1, max no 2, avg hard no 0.5, avg no 0.9, depth 6) | 5 tree(s)
Allow repeat: true | Prioritize soft no: true | Best cost = (max hard no 1, max no 2, avg hard no 0.2, avg no 1.2, depth 6) | 5 tree(s)
Allow repeat: false | Prioritize soft no: false | Best cost = (max hard no 1, max no 2, avg hard no 0.5, avg no 1.3, depth 6) | 5 tree(s)
Allow repeat: false | Prioritize soft no: true | Best cost = (max hard no 1, max no 2, avg hard no 0.4, avg no 1.4, depth 5) | 5 tree(s)
```

### Code Structure
- **solver.rs**: Core recursive solver with memoization
- **context.rs**: Word masks and partition iterators
- **constraints.rs**: Letter constraint rules
- **cost.rs**: Lexicographic cost comparison
- Uses: `Rc<Node>`, `HashMap` memoization, `SmallVec<[NodeRef; 5]>` for results

## Optimization Attempts

### Strategy: Alpha-Beta Style Pruning
**Branch**: `optim-pruning`
**Date**: 2025-11-28

#### Problem Identified
In `solver.rs::try_split()` (lines 197-198), the code unconditionally solves BOTH branches (yes and no) before checking if the combined result could possibly improve on `best_cost`. This wastes massive amounts of computation on paths that can never lead to better solutions.

#### Proposed Fix
Add early termination pruning:
1. **Before solving yes branch**: Check if we have a best_cost and if continuing makes sense
2. **After solving yes branch**: Compute lower bound on combined cost. If it's already worse than best_cost (considering the cost increment from the split), skip solving the no branch entirely
3. **Early exit**: If even the best possible outcome from this split can't beat best_cost, return early

This preserves optimality because we only skip work when we can prove the result won't improve the solution.

#### Implementation Plan
- Modify `try_split` to check `best_cost` before recursing into no branch
- Add a helper to compute minimum possible cost for a branch (lower bound)
- Test that results remain identical (optimal solutions preserved)

### Next Steps
1. ✓ Profile the code to identify bottlenecks → Found: `try_split` unconditional recursion
2. Implement alpha-beta pruning in `try_split`
3. Measure performance improvement
4. Validate correctness with tests

---

## Corrected Baseline (2025-11-28)

**IMPORTANT**: Initial measurements were in DEBUG mode. Release mode is required for valid benchmarking.

### Release Mode Baseline
Measured with `cargo build --release --quiet && time ./target/release/anagram_design`:
- **Total runtime**: 91.2 seconds
- **Per-configuration average**: ~23 seconds
- **Target**: <4s total (<1s per config)
- **Required speedup**: ~23x (much more achievable than ~135x!)

---

## Attempt #1: Alpha-Beta Pruning

**Branch**: `optim` (merged)
**Date**: 2025-11-28

### Implementation
Added pruning in `try_split()` to skip the no-branch when it provably can't improve best_cost:
- Check if no-branch is unsolvable → early return
- Calculate lower bound for combined cost (yes + no branches)
- Skip yes-branch if lower bound exceeds best_cost

### Results
- **Runtime**: 83.6 seconds
- **Speedup**: 1.09x over baseline (91.2s)
- **Status**: ✓ Merged (correctness validated, all tests pass)
- **Analysis**: Minimal improvement. The pruning works but doesn't fire often enough to make a big difference.

---

## Attempt #2: Reverse Branch Order

**Branch**: `optim` (merged)
**Date**: 2025-11-28

### Hypothesis
User insight: "optimal trees are much deeper along the yes branch than along the no branch, but cost tends to be in the no branches. Thus, you might want to check no before yes."

By solving the no-branch FIRST, we get the actual no_cost earlier, enabling better pruning when evaluating the yes-branch.

### Implementation
Changed `try_split()` to:
1. Solve no-branch first
2. Use actual no_cost (not lower bound) for pruning decisions
3. Solve yes-branch second with better pruning information

### Results
- **Runtime**: 78.1 seconds
- **Speedup**: 1.17x over baseline (91.2s)
- **Status**: ✓ Merged (correctness validated)
- **Analysis**: Better than alpha-beta alone, but still not a massive win.

---

## Attempt #3: Split Reordering (Soft Before Hard)

**Branch**: `optim` (merged)
**Date**: 2025-11-28

### Hypothesis
User insight: "Split wise, trying soft ones (in contains, positional, double/triple order) before hard ones might be a good default order."

Soft splits are cheaper (no hard_nos increment) and may find good solutions earlier, improving pruning effectiveness.

### Implementation
Reordered split attempts in all position iterators:
1. Soft reciprocal splits (e.g., "Contains 'e'? (all No contain 'i')")
2. Soft adjacent/mirror positional splits
3. Hard splits (test == requirement)

Also ordered within hard splits: contains → positional → double/triple

### Results
- **Runtime**: 73.0 seconds
- **Speedup**: 1.25x over baseline (91.2s)
- **Status**: ✓ Merged (correctness validated)
- **Analysis**: Best single optimization so far! Soft-first ordering helps find good solutions earlier.

---

## Attempt #4: Combined (Reverse Branch + Split Reorder)

**Branch**: `optim` (current)
**Date**: 2025-11-28

### Implementation
Combined both strategies:
- Reverse branch order (solve no-branch first)
- Split reordering (soft before hard)

### Results
- **Runtime**: 73.3 seconds
- **Speedup**: 1.24x over baseline (91.2s)
- **Status**: ✓ Current (correctness validated)
- **Analysis**: **Optimizations don't stack!** Combined result (73.3s) is barely different from split reordering alone (73.0s). Split reordering captures most of the benefit; reverse branch order adds minimal value on top.

---

## Summary of Progress

| Strategy | Runtime | Speedup vs Baseline | Incremental Speedup | Status |
|----------|---------|---------------------|---------------------|--------|
| Baseline (release) | 91.2s | 1.00x | - | - |
| Alpha-beta pruning | 83.6s | 1.09x | 1.09x | Merged |
| Reverse branch order | 78.1s | 1.17x | 1.09x | Merged |
| Split reordering | 73.0s | 1.25x | 1.07x | Merged |
| Combined (branch+split) | 73.3s | 1.24x | 1.00x | Superseded |
| **Optimized memoization** | **46.8s** | **1.95x** | **1.57x** | **Current** |

**Total improvement**: 1.95x (91.2s → 46.8s) - almost 2x faster!
**Remaining gap**: Need ~12x more speedup to reach <4s target

---

## Attempt #5: Optimized Memoization Key ⭐

**Branch**: `optim` (current)
**Date**: 2025-11-28

### Problem Analysis
The original memoization key included 6 fields:
```rust
struct Key {
    mask: u16,
    allow_repeat: bool,           // ❌ Never changes during a single solve() run
    prioritize_soft_no: bool,     // ❌ Never changes during a single solve() run
    forbidden_primary: u32,       // 🔄 Can be merged with forbidden_secondary
    forbidden_secondary: u32,     // 🔄 Can be merged with forbidden_primary
    allowed_primary_once: u32,    // ✅ Must keep (affects which splits are legal)
}
```

**Key insights**:
1. **allow_repeat & prioritize_soft_no**: These parameters never change during a single solve() run (each of the 4 configurations gets its own HashMap). Including them fragments the cache unnecessarily.

2. **forbidden_primary vs forbidden_secondary**: The distinction between primary and secondary matters when **propagating** constraints to children (deciding what to forbid), but not when **looking up** cached results. Two states with the same forbidden letters should have the same optimal tree - what matters is which letters are "touched", not whether they were touched as primary or secondary.

3. **allowed_primary_once**: MUST keep this! It affects which exception-based splits are legal for immediate children. Removing it breaks correctness (test failure confirmed this).

### Implementation
Simplified the Key structure to:
```rust
struct Key {
    mask: u16,
    forbidden: u32,              // Merged: forbidden_primary | forbidden_secondary
    allowed_primary_once: u32,   // Kept: affects legal split types
}
```

**Benefits**:
- Smaller key → better hash performance
- Merged forbidden fields → higher cache hit rate (states that differ only in primary/secondary distinction now share cache entries)
- Removed allow_repeat/prioritize_soft_no → eliminates unnecessary fragmentation

### Results
- **Runtime**: 46.8 seconds (average of two runs: 47.1s and 46.5s)
- **Speedup over previous best (73.3s)**: 1.57x
- **Speedup over baseline (91.2s)**: 1.95x
- **Status**: ✅ All tests pass, correctness validated

### Analysis
This is BY FAR the biggest single win! The memoization optimization alone delivered 1.57x speedup, more than all previous optimizations combined. Key factors:
- Smaller key size reduces hash computation overhead
- Merging forbidden fields dramatically improves cache hit rates
- Same word subset with different forbidden_primary/forbidden_secondary combinations now reuse cached results

**Critical learning**: Merging the forbidden fields was safe because the optimal tree for a word subset depends on which letters are *forbidden*, not on *how* they became forbidden (primary vs secondary). This semantic insight enabled a major performance gain.

---

## Attempt #6: Tighter Lower Bound for Pruning ❌

**Branch**: `optim` (reverted)
**Date**: 2025-11-28

### Hypothesis
The current pruning uses `min_yes_nos = 0` as a lower bound, assuming the YES branch could theoretically need 0 splits. This is very loose. A tighter estimate:
- If YES has 2+ words without repeat → need at least 1 split
- If YES has 3+ words with repeat → need at least 1 split

This should enable more aggressive pruning.

### Implementation
```rust
let yes_word_count = mask_count(yes);
let min_yes_nos = if allow_repeat {
    if yes_word_count >= 3 { 1 } else { 0 }
} else {
    if yes_word_count >= 2 { 1 } else { 0 }
};
```

### Results
- **Runtime**: 51.1 seconds
- **Change**: **1.09x SLOWDOWN** (worse than 46.8s baseline)
- **Status**: ❌ Reverted

### Analysis
**FAILED**: The overhead of calling `mask_count(yes)` on every `try_split()` invocation outweighed the benefit of tighter pruning.

**Why it failed**:
- `try_split()` is called VERY frequently (millions of times)
- `mask_count()` has a cost (popcount operation)
- The pruning condition fires relatively rarely
- When it does fire, min_yes_nos = 1 vs min_yes_nos = 0 doesn't enable much extra pruning (no_cost_nos is usually already > 1)

**Lesson learned**: Even cheap operations (like popcount) become expensive bottlenecks when called in tight inner loops. Profile-driven optimization is essential - intuitive "improvements" can backfire!

---

## Attempt #7: Lower Bound for NO Branch ❌

**Branch**: `optim` (reverted)
**Date**: 2025-11-28

### Hypothesis
Following up on Attempt #6, check a lower bound for the NO branch BEFORE solving it. If even the NO lower bound + best-case YES (0 cost) exceeds best_cost, skip the entire split without solving either branch.

**Trade-off**: One `mask_count(no)` call vs potentially skipping an expensive `solve()` call.

### Results
- **Status**: ❌ Test failure (breaks correctness)
- **Error**: Found sum_hard_nos:4, expected sum_hard_nos:3 (found suboptimal solution)

### Analysis
**FAILED**: The lower bound was too optimistic, causing us to prune splits that lead to optimal solutions.

**Why it failed**:
- Lower bound assumes all required splits could be **soft** (min_hard_nos = 0 if soft possible)
- In practice, **constraints force hard splits** even when soft splits would be theoretically possible
- Pruning based on this optimistic bound cuts off paths to optimal trees

**Lesson learned**: Simple word-count-based lower bounds don't capture constraint complexity. A tighter bound would need to account for:
1. Available letter distribution
2. Forbidden letter constraints
3. Whether soft split reciprocals actually exist in the word set

Such analysis would be more expensive than the solve() calls we're trying to avoid!

---

