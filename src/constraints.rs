#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Constraints {
    /// Letters forbidden as primary letters in this subtree
    pub forbidden_primary: u32,
    /// Letters forbidden as secondary letters in this subtree
    pub forbidden_secondary: u32,
    /// Letters that are temporarily allowed as primary for the *first* split in this subtree
    /// (used for the contain exceptions)
    pub allowed_primary_once: u32,
}

impl Constraints {
    pub fn empty() -> Self {
        Constraints {
            forbidden_primary: 0,
            forbidden_secondary: 0,
            allowed_primary_once: 0,
        }
    }

    pub fn primary_allowed(&self, idx: usize) -> bool {
        let bit = 1u32 << idx;
        (self.forbidden_primary & bit == 0) || (self.allowed_primary_once & bit != 0)
    }

    pub fn secondary_allowed(&self, idx: usize) -> bool {
        let bit = 1u32 << idx;
        self.forbidden_secondary & bit == 0
    }

    /// Clear one-time allowances when descending; persistent forbiddances stay.
    pub fn next_level(&self) -> Self {
        Constraints {
            forbidden_primary: self.forbidden_primary,
            forbidden_secondary: self.forbidden_secondary,
            allowed_primary_once: 0,
        }
    }

    pub fn prune(self, present_letters: u32) -> Self {
        Constraints {
            forbidden_primary: self.forbidden_primary & present_letters,
            forbidden_secondary: self.forbidden_secondary & present_letters,
            allowed_primary_once: self.allowed_primary_once & present_letters,
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
];

pub fn split_allowed(constraints: &Constraints, primary_idx: usize, secondary_idx: usize) -> bool {
    constraints.primary_allowed(primary_idx) && constraints.secondary_allowed(secondary_idx)
}

pub fn branch_constraints(
    constraints: &Constraints,
    primary_idx: usize,
    secondary_idx: usize,
    yes_primary_allow: Option<u32>,
    no_primary_allow: Option<u32>,
) -> (Constraints, Constraints) {
    let mut yes = constraints.next_level();
    let mut no = constraints.next_level();

    let primary_bit = 1u32 << primary_idx;
    let secondary_bit = 1u32 << secondary_idx;

    // Apply the general rule
    yes.forbidden_primary |= primary_bit;
    yes.forbidden_secondary |= primary_bit;

    no.forbidden_primary |= primary_bit | secondary_bit;
    no.forbidden_secondary |= primary_bit | secondary_bit;

    // Exception allowances (single-use)
    if let Some(bit) = yes_primary_allow {
        yes.allowed_primary_once |= bit;
    }
    if let Some(bit) = no_primary_allow {
        no.allowed_primary_once |= bit;
    }

    (yes, no)
}
