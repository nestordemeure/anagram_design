pub struct Context<'a> {
    pub words: &'a [String],
    pub letter_masks: [u16; 26],
    pub first_letter_masks: [u16; 26],
    pub second_letter_masks: [u16; 26],
    pub third_letter_masks: [u16; 26],
    pub last_letter_masks: [u16; 26],
    pub second_to_last_letter_masks: [u16; 26],
    pub third_to_last_letter_masks: [u16; 26],
    pub double_letter_masks: [u16; 26],
    pub triple_letter_masks: [u16; 26],
    pub global_letters: Vec<usize>, // Precomputed letters present in word set
}

impl<'a> Context<'a> {
    pub fn new(words: &'a [String]) -> Self {
        let letter_masks = make_letter_masks(words);
        let mut global_letters = Vec::with_capacity(26);
        for idx in 0..26 {
            if letter_masks[idx] != 0 {
                global_letters.push(idx);
            }
        }
        Context {
            words,
            letter_masks,
            first_letter_masks: make_first_letter_masks(words),
            second_letter_masks: make_second_letter_masks(words),
            third_letter_masks: make_third_letter_masks(words),
            last_letter_masks: make_last_letter_masks(words),
            second_to_last_letter_masks: make_second_to_last_letter_masks(words),
            third_to_last_letter_masks: make_third_to_last_letter_masks(words),
            double_letter_masks: make_double_letter_masks(words),
            triple_letter_masks: make_triple_letter_masks(words),
            global_letters,
        }
    }
}

pub fn mask_count(mask: u16) -> u32 {
    mask.count_ones()
}

pub fn position_mask(ctx: &Context<'_>, from_end: bool, pos_index: u8, letter_idx: usize) -> u16 {
    match (from_end, pos_index) {
        (false, 1) => ctx.first_letter_masks[letter_idx],
        (false, 2) => ctx.second_letter_masks[letter_idx],
        (false, 3) => ctx.third_letter_masks[letter_idx],
        (true, 1) => ctx.last_letter_masks[letter_idx],
        (true, 2) => ctx.second_to_last_letter_masks[letter_idx],
        (true, 3) => ctx.third_to_last_letter_masks[letter_idx],
        _ => 0,
    }
}

pub fn single_word_from_mask(mask: u16, words: &[String]) -> Option<String> {
    let idx = mask.trailing_zeros() as usize;
    if idx < words.len() {
        Some(words[idx].clone())
    } else {
        None
    }
}

/// Return all letter indices that produce a true partition of `mask` with the given per-letter masks.
/// Each item is (letter_index, yes_mask, no_mask).
pub struct Partitions<'a> {
    masks: &'a [u16; 26],
    mask: u16,
    global_letters: &'a [usize],
    idx: usize,
}

impl<'a> Iterator for Partitions<'a> {
    type Item = (usize, u16, u16);

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.global_letters.len() {
            let letter_idx = self.global_letters[self.idx];
            self.idx += 1;
            let letter_mask = self.masks[letter_idx];
            let yes = self.mask & letter_mask;
            if yes == 0 || yes == self.mask {
                continue;
            }
            let no = self.mask & !letter_mask;
            return Some((letter_idx, yes, no));
        }
        None
    }
}

pub fn partitions<'a>(mask: u16, masks: &'a [u16; 26], global_letters: &'a [usize]) -> Partitions<'a> {
    Partitions {
        masks,
        mask,
        global_letters,
        idx: 0,
    }
}

pub fn letters_present(mask: u16, ctx: &Context<'_>) -> u32 {
    let mut present: u32 = 0;
    for idx in 0..26 {
        if mask & ctx.letter_masks[idx] != 0 {
            present |= 1u32 << idx;
        }
    }
    present
}

fn make_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_first_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().next() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_second_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().nth(1) {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_third_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().nth(2) {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_last_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        if let Some(ch) = w.chars().last() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_second_to_last_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let chars: Vec<char> = w.chars().collect();
        if chars.len() >= 2 {
            let ch = chars[chars.len() - 2];
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_third_to_last_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let chars: Vec<char> = w.chars().collect();
        if chars.len() >= 3 {
            let ch = chars[chars.len() - 3];
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_double_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let mut counts = [0u8; 26];
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                if counts[l] < 3 {
                    counts[l] += 1;
                }
            }
        }
        for (l, &c) in counts.iter().enumerate() {
            if c >= 2 {
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}

fn make_triple_letter_masks(words: &[String]) -> [u16; 26] {
    let mut masks = [0u16; 26];
    for (idx, w) in words.iter().enumerate() {
        let mut counts = [0u8; 26];
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                let l = ch.to_ascii_lowercase() as usize - 'a' as usize;
                if counts[l] < 3 {
                    counts[l] += 1;
                }
            }
        }
        for (l, &c) in counts.iter().enumerate() {
            if c >= 3 {
                masks[l] |= 1u16 << idx;
            }
        }
    }
    masks
}
