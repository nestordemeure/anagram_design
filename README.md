# Annagram Design

## Theory

An Annagram is a tree that organize a set of words.

Leaves each have a single word.

Nodes have a Yes children, and a No children.
The test for the presence of a letter, if it is in a word then the word goes in the Yes children, otherwise in the No children.

The cost of a tree is `(1,0) + max(cost(Yes), cost(No))`

A leaf, with a single node, has a cost of `(0,0)` (as would a leaf with no words, but those are not expected to exist)

Their is a special `Repeat` node type for sets of two words, it has a cost of `(0,1)`.

## Goal

Put together an algorithm that can take a set of words and create an annagram tree for them, minimizing cost.

We want it to be equiped to display then as a `tree`-like output.

Ideally we want to be able to display *all* trees that have the minimum cost (if there is more than one).

Our test set of words will be the 12 signs of the Zodiac (english spelling).

## TODO

* add a python gitignore
* write the general algorithm
* write the Zodiac test
* run and test it