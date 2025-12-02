use std::rc::Rc;

use crate::cost::Cost;

/// Represents the position/type of a split
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum Position {
    Contains,
    First,
    Second,
    Third,
    ThirdToLast,
    SecondToLast,
    Last,
    Double,
    Triple,
}

impl Position {
    pub const fn name(&self) -> &'static str {
        match self {
            Position::Contains => "contains",
            Position::First => "first",
            Position::Second => "second",
            Position::Third => "third",
            Position::ThirdToLast => "third-to-last",
            Position::SecondToLast => "second-to-last",
            Position::Last => "last",
            Position::Double => "double",
            Position::Triple => "triple",
        }
    }

    /// Convert this position to an absolute index (0-based) for a word of given length.
    /// Returns None if the word is too short for this position or if the position is not positional.
    pub const fn to_absolute_index(&self, word_length: usize) -> Option<usize> {
        match *self {
            Position::Contains | Position::Double | Position::Triple => None,  // Not positional
            Position::First => if word_length >= 1 { Some(0) } else { None },
            Position::Second => if word_length >= 2 { Some(1) } else { None },
            Position::Third => if word_length >= 3 { Some(2) } else { None },
            Position::Last => if word_length >= 1 { Some(word_length - 1) } else { None },
            Position::SecondToLast => if word_length >= 2 { Some(word_length - 2) } else { None },
            Position::ThirdToLast => if word_length >= 3 { Some(word_length - 3) } else { None },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Leaf(String),
    /// Ask directly for a specific word; Yes resolves that word, No continues with the rest.
    Repeat {
        word: String,
        no: Rc<Node>,
    },
    /// Unified positional split that handles all split types
    PositionalSplit {
        /// Letter to test for (primary letter)
        test_letter: char,
        /// Position where to test
        test_position: Position,
        /// Letter required in No branch (secondary letter)
        /// For hard splits, this is the same as test_letter
        requirement_letter: char,
        /// Position where requirement is checked
        /// For hard splits, this is the same as test_position
        requirement_position: Position,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
}

pub type NodeRef = Rc<Node>;

#[derive(Debug, Clone)]
pub struct Solution {
    pub cost: Cost,
    pub trees: Vec<NodeRef>,
}

impl Solution {
    /// Check if this solution is unsolvable (no valid trees)
    pub const fn is_unsolvable(&self) -> bool {
        self.trees.is_empty()
    }

    /// Create an unsolvable solution with worst-possible cost
    pub const fn unsolvable(word_count: u32) -> Self {
        Solution {
            cost: Cost {
                hard_nos: u32::MAX,
                redeemed_hard_nos: i32::MAX,
                nos: u32::MAX,
                redeemed_nos: i32::MAX,
                sum_hard_nos: u32::MAX,
                redeemed_sum_hard_nos: i32::MAX,
                sum_nos: u32::MAX,
                redeemed_sum_nos: i32::MAX,
                word_count,
            },
            trees: Vec::new(),
        }
    }
}

/// Create a positional split node
pub fn combine_positional_split(
    test_letter: char,
    test_position: Position,
    requirement_letter: char,
    requirement_position: Position,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::PositionalSplit {
        test_letter,
        test_position,
        requirement_letter,
        requirement_position,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

/// Helper to determine if a split is hard (same test and requirement)
pub fn is_hard_split(test_letter: char, test_position: Position, requirement_letter: char, requirement_position: Position) -> bool {
    test_letter == requirement_letter && test_position == requirement_position
}
