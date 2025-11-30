# Redeeming Hits

## Rough Theory

Whenever we get a `yes`, that's a `hit`.

Let's introduce the concept of `redeeming hits` (in the UI, where they are set at a default of 2, and in the code).
If you get `n` redeeming hits (ou defualt of 2) right after a no, then that no is not counted.
Obviously no nos would be even better, so we need to introduce a third counter to the cost (after hard no, and no) to track true nos (same with averages). 

Let's introduce `HitSplits`.
They are copies of the hard splits, and come with the same constraints down the line, but all words go into their yes branch (ie a "contain A" split used when all words contain A).
Thus, adding one does not introduce any no, and gets you a hit.

Our current search algorithm might not be amenable to that possibility of decreasing costs.
We have to change it from a dikjstra like to something else maybe?

the value returned by a subtree could have a number of hits, capped at `n`
if the current node is a no, it turns those to 0
but, if those are `n`, then the current no does not count in the normal counts
that way we can still build a cost based on children's costs

## Local Reedeming Logic

Tree can return a number of hits, capped at `n`
the cost at the parent can be computed taking those into account, they become 0 on the max path (no)
a parent getting those from a no branch would set those to 0, 
now the true no (sum true no) in the cost would be different from no and sum no

## Implementation Plan

This is delicate to implement, thus we should do it in steps making sure things are valid before moving to the next thing.

* We can introduce `HitSplits` (or make them a valid option for hard splits)
  the solver will not see much point to them (as no redeeming hits logic is implemented, they are just no cost options) but that's fine
  we need to update the formater (both Rust and Javascript) to display nodes with no `no` branch

* We can introduce a `redeeming hits` parameter (`n`, defaults to 2, 0 means the redeeming hits logic is not enabled) to the UI
  it would be passed to the solver but, for now, do nothing.

* We can update the cost representation to introduce true nos, and sum true nos
  for now they will be identical to no and sum no
  the UI would still display true values below the tree, not redeeming (thus reduced) ones, even if that means recomputing them on a tree after the solve

Then we need to add the redeeming logic.

By this point our solver might return suboptimal results when `n>0`, due to the heuristic and logic of the solver being unfit for the task, but that's fine for now.
At this point we can start thinking about heuristic and solver logic.