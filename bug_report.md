# Constraint Bug (in relation to YesSplit logic)

## Settings

When solving the Zodiac set of words, with repeats false and prioritize soft no true, I get the following (merged) tree:

```
Contains 'R'? (all No contain 'E')
│└─ No: Second letter 'E'? (all No have 'E' second-to-last) ▼
│   │└─ No: Pisces
│   │
│   Contains 'L'? (all No contain 'I') ▼
│   │└─ No: Gemini
│   └─ Leo
│
Contains 'A'?
│└─ No: Contains 'I'? (yes only) ▼
│   │
│   Contains 'O'? (yes only) ▼
│   │
│   Second letter 'I'? (all No have 'I' second-to-last) ▼
│   │└─ No: Scorpio
│   └─ Virgo
│
Contains 'S'? ▼
│└─ No: Contains 'I'? (all No contain 'E')
│   │└─ No: Cancer
│   │
│   Contains 'B'? (all No contain 'P') ▼
│   │└─ No: Capricorn
│   └─ Libra
│
Contains 'T'? (all No contain 'I')
│└─ No: Third letter 'I'? (all No have 'I' third-to-last)
│   │└─ No: Aquarius
│   └─ Aries
│
Double 'I'? (all No double 'U') ▼
│└─ No: Taurus
│
└─ Sagittarius
```

The `Contains 'A'?` branch of the tree exhibits two constraints bugs.

## Constraint violation

Looking at this version of the subtree:

```
Contains 'A'?
│└─ No: Contains 'I'? (yes only) ▼
│   │
│   Contains 'O'? (yes only) ▼
│   │
│   Second letter 'I'? (all No have 'I' second-to-last) ▼
│   │└─ No: Scorpio
│   └─ Virgo
```

The use of `Contains 'I'? (yes only)` should make the letter `I` unuseable down the line exept for direct descendants following the exeption rules.

`Second letter 'I'? (all No have 'I' second-to-last)` should be forbidden, as it is not a direct descendent.

## Missed branch

Looking at this version of the subtree:

```
Contains 'A'?
│└─ No: Contains 'O'? (yes only) ▼
│   │
│   Contains 'I'? (yes only)
│   │
│   Contains 'G'? (all No contain 'C')
│   │└─ No: Scorpio
│   └─ Virgo
```

Here ``Second letter 'I'? (all No have 'I' second-to-last)`` should be legal (and of equal cost) where `Contains 'G'? (all No contain 'C')` is used.
But that laternative is not offered.

## Further notes

Those constraints violation appear specific to the use of Yessplits, and were not seen in the codebase before.

The fact that we see two different types of constraint problems highlit one of two likely problems:
* the constraints are not properly set / propagated into subtree solving 
* memoization's key is insuficiently precise leading us to pick a subtree that is unsuteable for our constraints