use std::rc::Rc;

use crate::cost::Cost;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Leaf(String),
    /// Ask directly for a specific word; Yes resolves that word, No continues with the rest.
    Repeat {
        word: String,
        no: Rc<Node>,
    },
    Split {
        letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftSplit {
        /// Letter to test for (e.g., 'i' in I/E)
        test_letter: char,
        /// Letter that all No items must contain (e.g., 'e' in I/E)
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    FirstLetterSplit {
        letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftFirstLetterSplit {
        /// Letter to test as first letter
        test_letter: char,
        /// Letter that all No items must have as second letter
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    LastLetterSplit {
        letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftLastLetterSplit {
        /// Letter to test as last letter
        test_letter: char,
        /// Letter that all No items must have as second-to-last letter
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftMirrorPosSplit {
        /// Letter to test
        test_letter: char,
        /// 1-based position from the tested end (1 = first/last)
        test_index: u8,
        /// true when counting from the end (last/second-to-last/third-to-last)
        test_from_end: bool,
        /// Position that all No items must carry the same letter in
        requirement_index: u8,
        /// true when the requirement position is counted from the end
        requirement_from_end: bool,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
    SoftDoubleLetterSplit {
        /// Letter that must appear twice in the Yes branch
        test_letter: char,
        /// Letter (different) that must appear twice in all No items
        requirement_letter: char,
        yes: Rc<Node>,
        no: Rc<Node>,
    },
}

pub type NodeRef = Rc<Node>;

#[derive(Debug, Clone)]
pub struct Solution {
    pub cost: Cost,
    pub trees: Vec<NodeRef>,
    pub exhausted: bool,
}

pub fn combine_children(letter: char, left: &NodeRef, right: &NodeRef) -> NodeRef {
    Rc::new(Node::Split {
        letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_soft_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_first_letter_children(letter: char, left: &NodeRef, right: &NodeRef) -> NodeRef {
    Rc::new(Node::FirstLetterSplit {
        letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_soft_first_letter_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftFirstLetterSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_last_letter_children(letter: char, left: &NodeRef, right: &NodeRef) -> NodeRef {
    Rc::new(Node::LastLetterSplit {
        letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_soft_last_letter_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftLastLetterSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_soft_mirror_pos_children(
    test_letter: char,
    test_index: u8,
    test_from_end: bool,
    requirement_index: u8,
    requirement_from_end: bool,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftMirrorPosSplit {
        test_letter,
        test_index,
        test_from_end,
        requirement_index,
        requirement_from_end,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}

pub fn combine_soft_double_letter_children(
    test_letter: char,
    requirement_letter: char,
    left: &NodeRef,
    right: &NodeRef,
) -> NodeRef {
    Rc::new(Node::SoftDoubleLetterSplit {
        test_letter,
        requirement_letter,
        yes: Rc::clone(left),
        no: Rc::clone(right),
    })
}
