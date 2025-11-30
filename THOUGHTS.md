# Thougts

## Grasping at thoughts

* the problem is 2 nos in a row
  we want yeses we can put in between those, even if they are not discriminant
  we could introduce a useless yes node type? and tweak the cost to prioritze it?

* some split have no impact, everything goes in their true branch
  * a function to add all of those that can be added legally might be fun for scripting purposes
  * also, they do soften the previous no / soft no (into no-, softno-)
    hard no < hard no- < sof no < soft no-
    a cost could be (hard no, hard no-, soft no, soft no-, avg etc)

maybe a concept we need is `hit`
or maybe `chain of hits`?
the problem with 2 nos in a row is that it puts our shortest chain of hits to 0 (arguable a soft no gets it to 0.5, and two soft nos in a row to.. 1? or 0.5?)

## Hits

We care about:
* hard no
* soft no
* hits

What does the worst path look like?
what does the average path look like?
how can i compare paths in a way that is satisfying?

how many nos are on it?
how many hits?
how many soft hits?

right now we compute:
worst path:
1. `hard_nos` — max hard No edges on any root→leaf path (component-wise max across branches)
2. `nos` — max No edges on any path
average path:
3. `sum_hard_nos` — weighted sum of hard No edges
4. `sum_nos` — weighted sum of No edges (words in the No branch each add 1)
further tree complexity metric:
5. `depth` — max tree depth
(inversing hard no and no depending on whether we want to get more soft nos, or less overall nos)

let me look at the paths in a tree:
soft soft pisces
soft hit soft gemini
soft hit hit leo
hit hard soft scorpio
hit hard hit virgo
hit hit hard soft cancer
hit hit hard hit soft libra
hit hit hard hit hit capricorn
hit hit hit soft aries
hit hit hit hit soft sagitarus
hit hit hit hit hit soft aquarius
hit hit hit hit hit hit taurus

lets trim starting and ending hits, plus cutting single questions hits:
soft soft
soft hit soft
hard soft
hard
hard soft
hard hit soft
hard
soft
soft
soft
here the worst is clearly `hard soft`, its the maximum number of nos in a row and has zero redeeming hit
if we look at a single element, `hard` is clearly worse than `soft`
and `hard hit soft` is clearly better than `hard soft`
what about `soft hard` vs `hard soft`? no strong position on that.
what about `hard soft` vs `hard hit hit soft hit hit soft`? no wthat is interesting ... i might prefer the later...

it sounds like the concept of `redeeming hits` might be meaningful
`n` (default to 2) hits after a no redeem it, erasing it from the slate
that, however, should still be worst than no no (which eans that, while we decrease the main no counter, we have one after the no and soft no that is no decreased, same for averages)
but better than just a no, or even a soft

## Theory

Whenever we get a `yes`, that's a `hit`.

Let's introduce the concept of `redeeming hits` (in the UI, where they are set at a default of 2, and in the code).
If you get `n` redeeming hits (ou defualt of 2) right after a no, then that no is not counted.
Obviously no nos would be even better, so we need to introduce a third counter (after hard no, and no) to track true nos (same with averages). 

Let's introduce `HitSplits`.
They are copies of the hard splits, and come with the same constraints down the line, but all words go into their yes branch (ie a "contain A" split used when all words contain A).
Thus, adding one does not introduce any no, and gets you a hit.

Our current search algorithm might not be amenable to that possibility of decreasing costs.
We have to change it from a dikjstra like to something else...