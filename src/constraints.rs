use crate::node::Position;

/// Split classes for constraint exceptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SplitClass {
    Contains = 0,
    Positional = 1,
    DoubleTriple = 2,
}

/// Get the class of a position
pub const fn position_class(pos: Position) -> SplitClass {
    match pos {
        Position::Contains => SplitClass::Contains,
        Position::First | Position::Second | Position::Third |
        Position::ThirdToLast | Position::SecondToLast | Position::Last => SplitClass::Positional,
        Position::Double | Position::Triple => SplitClass::DoubleTriple,
    }
}

/// Check if a child can use the parent's letter based on class movement
/// (same-class or downward: Contains -> Positional -> DoubleTriple)
pub fn can_chain_exception(parent_pos: Position, child_pos: Position) -> bool {
    position_class(child_pos) >= position_class(parent_pos)
}

/// Check if two positions could refer to the same absolute index for any reasonable word length.
/// This prevents chaining like "Second E" -> "Second-to-last E" on 3-letter words where both
/// positions refer to index 1.
pub fn positions_can_collide(pos1: Position, pos2: Position) -> bool {
    // Only positional splits can collide (Contains, Double, Triple are not positional)
    if matches!(pos1, Position::Contains | Position::Double | Position::Triple) {
        return false;
    }
    if matches!(pos2, Position::Contains | Position::Double | Position::Triple) {
        return false;
    }

    // Check all reasonable word lengths (1-20 covers all practical cases)
    for len in 1..=20 {
        if let (Some(idx1), Some(idx2)) = (pos1.to_absolute_index(len), pos2.to_absolute_index(len)) {
            if idx1 == idx2 {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Constraints {
    /// Letters forbidden as primary letters in this subtree
    pub forbidden_primary: u32,
    /// Letters forbidden as secondary letters in this subtree
    pub forbidden_secondary: u32,
    /// Letters that are temporarily allowed as primary for the *first* split in this subtree
    /// (used for the contain exceptions)
    pub allowed_primary_once: u32,
    /// The position of the parent split (for determining if exceptions can chain)
    pub parent_position: Option<Position>,
    /// The letter from the parent split that can chain in this branch
    pub parent_letter: Option<usize>,
}

impl Constraints {
    pub const fn empty() -> Self {
        Constraints {
            forbidden_primary: 0,
            forbidden_secondary: 0,
            allowed_primary_once: 0,
            parent_position: None,
            parent_letter: None,
        }
    }

    pub fn primary_allowed(&self, idx: usize, child_pos: Position) -> bool {
        let bit = 1u32 << idx;

        // Check if not forbidden (always allowed)
        if self.forbidden_primary & bit == 0 {
            return true;
        }

        // Check if allowed via immediate-child exception (must verify class movement)
        if self.allowed_primary_once & bit != 0 {
            // Verify class movement is valid (same-class or downward)
            if let Some(parent_pos) = self.parent_position {
                if can_chain_exception(parent_pos, child_pos) {
                    return true;
                }
            } else {
                // No parent info means root level, allow for backward compatibility
                return true;
            }
        }

        // Check for chaining exceptions (for continuing chains)
        if let (Some(parent_pos), Some(parent_letter)) = (self.parent_position, self.parent_letter) {
            if idx == parent_letter && can_chain_exception(parent_pos, child_pos) {
                // Disallow if positions would refer to the same absolute index
                if positions_can_collide(parent_pos, child_pos) {
                    return false;
                }
                return true;
            }
        }

        false
    }

    pub const fn secondary_allowed(&self, idx: usize) -> bool {
        let bit = 1u32 << idx;
        self.forbidden_secondary & bit == 0
    }

    /// Clear one-time allowances when descending; persistent forbiddances stay.
    pub const fn next_level(&self) -> Self {
        Constraints {
            forbidden_primary: self.forbidden_primary,
            forbidden_secondary: self.forbidden_secondary,
            allowed_primary_once: 0,
            parent_position: self.parent_position,
            parent_letter: self.parent_letter,
        }
    }

    pub const fn prune(self, present_letters: u32) -> Self {
        Constraints {
            forbidden_primary: self.forbidden_primary & present_letters,
            forbidden_secondary: self.forbidden_secondary & present_letters,
            allowed_primary_once: self.allowed_primary_once & present_letters,
            parent_position: self.parent_position,
            parent_letter: self.parent_letter,
        }
    }
}

/// Defines a soft no pair: (test_letter, requirement_letter)
/// E/I means: test for 'e', require all No items contain 'i'
/// Children cannot use any soft no containing either letter
#[derive(Debug, Clone, Copy)]
pub struct SoftNoPair {
    /// Test for this letter
    pub test_letter: char,
    /// Require all No items contain this letter
    pub requirement_letter: char,
}

/// Define the available soft no pairs
/// Children of a soft no cannot use any soft no containing either letter
pub const SOFT_NO_PAIRS: &[SoftNoPair] = &[
    // E/I pair - vowel similarity
    SoftNoPair {
        test_letter: 'e',
        requirement_letter: 'i',
    },
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 'e',
    },
    // C/K pair - identical hard sound
    SoftNoPair {
        test_letter: 'c',
        requirement_letter: 'k',
    },
    SoftNoPair {
        test_letter: 'k',
        requirement_letter: 'c',
    },
    // S/Z pair - similar sibilants
    SoftNoPair {
        test_letter: 's',
        requirement_letter: 'z',
    },
    SoftNoPair {
        test_letter: 'z',
        requirement_letter: 's',
    },
    // I/L pair - visually similar
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 'l',
    },
    SoftNoPair {
        test_letter: 'l',
        requirement_letter: 'i',
    },
    // M/N pair - nasals
    SoftNoPair {
        test_letter: 'm',
        requirement_letter: 'n',
    },
    SoftNoPair {
        test_letter: 'n',
        requirement_letter: 'm',
    },
    // U/V pair - visually similar
    SoftNoPair {
        test_letter: 'u',
        requirement_letter: 'v',
    },
    SoftNoPair {
        test_letter: 'v',
        requirement_letter: 'u',
    },
    // O/Q pair - visually similar
    SoftNoPair {
        test_letter: 'o',
        requirement_letter: 'q',
    },
    SoftNoPair {
        test_letter: 'q',
        requirement_letter: 'o',
    },
    // C/G pair - visually similar
    SoftNoPair {
        test_letter: 'c',
        requirement_letter: 'g',
    },
    SoftNoPair {
        test_letter: 'g',
        requirement_letter: 'c',
    },
    // B/P pair - voiced/unvoiced
    SoftNoPair {
        test_letter: 'b',
        requirement_letter: 'p',
    },
    SoftNoPair {
        test_letter: 'p',
        requirement_letter: 'b',
    },
    // I/T pair - visually similar
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 't',
    },
    SoftNoPair {
        test_letter: 't',
        requirement_letter: 'i',
    },
    // R/E pair
    SoftNoPair {
        test_letter: 'r',
        requirement_letter: 'e',
    },
    SoftNoPair {
        test_letter: 'e',
        requirement_letter: 'r',
    },
    // A/R pair - similar open shapes in block capitals
    SoftNoPair {
        test_letter: 'a',
        requirement_letter: 'r',
    },
    SoftNoPair {
        test_letter: 'r',
        requirement_letter: 'a',
    },
    // I/J pair
    SoftNoPair {
        test_letter: 'i',
        requirement_letter: 'j',
    },
    SoftNoPair {
        test_letter: 'j',
        requirement_letter: 'i',
    },
    // V/W pair
    SoftNoPair {
        test_letter: 'v',
        requirement_letter: 'w',
    },
    SoftNoPair {
        test_letter: 'w',
        requirement_letter: 'v',
    },
    // Q/G pair
    SoftNoPair {
        test_letter: 'q',
        requirement_letter: 'g',
    },
    SoftNoPair {
        test_letter: 'g',
        requirement_letter: 'q',
    },
    // E/B pair
    SoftNoPair {
        test_letter: 'e',
        requirement_letter: 'b',
    },
    SoftNoPair {
        test_letter: 'b',
        requirement_letter: 'e',
    },
    // E/F pair
    SoftNoPair {
        test_letter: 'e',
        requirement_letter: 'f',
    },
    SoftNoPair {
        test_letter: 'f',
        requirement_letter: 'e',
    },
    // R/P pair
    SoftNoPair {
        test_letter: 'r',
        requirement_letter: 'p',
    },
    SoftNoPair {
        test_letter: 'p',
        requirement_letter: 'r',
    },
    // R/B pair
    SoftNoPair {
        test_letter: 'r',
        requirement_letter: 'b',
    },
    SoftNoPair {
        test_letter: 'b',
        requirement_letter: 'r',
    },
    // T/F pair
    SoftNoPair {
        test_letter: 't',
        requirement_letter: 'f',
    },
    SoftNoPair {
        test_letter: 'f',
        requirement_letter: 't',
    },
    // Y/X pair
    SoftNoPair {
        test_letter: 'y',
        requirement_letter: 'x',
    },
    SoftNoPair {
        test_letter: 'x',
        requirement_letter: 'y',
    },
    // Y/V pair
    SoftNoPair {
        test_letter: 'y',
        requirement_letter: 'v',
    },
    SoftNoPair {
        test_letter: 'v',
        requirement_letter: 'y',
    },
    // O/G pair
    SoftNoPair {
        test_letter: 'o',
        requirement_letter: 'g',
    },
    SoftNoPair {
        test_letter: 'g',
        requirement_letter: 'o',
    },
    // P/F pair
    SoftNoPair {
        test_letter: 'p',
        requirement_letter: 'f',
    },
    SoftNoPair {
        test_letter: 'f',
        requirement_letter: 'p',
    },
    // A/H pair
    SoftNoPair {
        test_letter: 'a',
        requirement_letter: 'h',
    },
    SoftNoPair {
        test_letter: 'h',
        requirement_letter: 'a',
    },
    // D/B pair
    SoftNoPair {
        test_letter: 'd',
        requirement_letter: 'b',
    },
    SoftNoPair {
        test_letter: 'b',
        requirement_letter: 'd',
    },
    // J/L pair
    SoftNoPair {
        test_letter: 'j',
        requirement_letter: 'l',
    },
    SoftNoPair {
        test_letter: 'l',
        requirement_letter: 'j',
    },
];

pub fn split_allowed(
    constraints: &Constraints,
    primary_idx: usize,
    secondary_idx: usize,
    position: Position,
) -> bool {
    // For hard splits (primary == secondary), the exception should apply to both checks
    if primary_idx == secondary_idx {
        constraints.primary_allowed(primary_idx, position)
    } else {
        constraints.primary_allowed(primary_idx, position) && constraints.secondary_allowed(secondary_idx)
    }
}

/// Get the reciprocal letter index for a given letter, if one exists.
/// Returns None if the letter has no defined reciprocal.
pub fn get_reciprocal(letter_idx: usize) -> Option<usize> {
    let letter = (b'a' + letter_idx as u8) as char;

    // Find a soft no pair where this letter is the test_letter
    for pair in SOFT_NO_PAIRS {
        if pair.test_letter == letter {
            return Some((pair.requirement_letter as u8 - b'a') as usize);
        }
    }

    None
}

pub const fn branch_constraints(
    constraints: &Constraints,
    primary_idx: usize,
    secondary_idx: usize,
    position: Position,
    yes_primary_allow: Option<u32>,
    no_primary_allow: Option<u32>,
) -> (Constraints, Constraints) {
    let mut yes = constraints.next_level();
    let mut no = constraints.next_level();

    let primary_bit = 1u32 << primary_idx;
    let secondary_bit = 1u32 << secondary_idx;

    // Apply the general rule: touched letters are forbidden
    // In yes branch: primary is touched
    yes.forbidden_primary |= primary_bit;
    yes.forbidden_secondary |= primary_bit;

    // In no branch: both primary and secondary are touched
    no.forbidden_primary |= primary_bit | secondary_bit;
    no.forbidden_secondary |= primary_bit | secondary_bit;

    // Store parent info for chaining exceptions
    // Yes branch: primary is touched (but can chain), secondary is untouched
    yes.parent_position = Some(position);
    yes.parent_letter = Some(primary_idx);

    // No branch: both are touched (secondary can chain in soft splits)
    no.parent_position = Some(position);
    no.parent_letter = if primary_idx != secondary_idx {
        // Only in soft splits can the secondary chain in no branch
        Some(secondary_idx)
    } else {
        None
    };

    // Exception allowances (single-use, for immediate children only)
    if let Some(bit) = yes_primary_allow {
        yes.allowed_primary_once |= bit;
    }
    if let Some(bit) = no_primary_allow {
        no.allowed_primary_once |= bit;
    }

    (yes, no)
}
