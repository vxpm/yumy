use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const TAB: &str = "\t";
const ZERO_WIDTH_JOINER: &str = "\u{200d}";
const VARIATION_SELECTOR_16: &str = "\u{fe0f}";
const SKIN_TONES: [&str; 5] = [
    "\u{1f3fb}", // Light Skin Tone
    "\u{1f3fc}", // Medium-Light Skin Tone
    "\u{1f3fd}", // Medium Skin Tone
    "\u{1f3fe}", // Medium-Dark Skin Tone
    "\u{1f3ff}", // Dark Skin Tone
];

/// Returns the display width of a grapheme. This function _does not_ assert that
/// the argument is indeed a single grapheme and therefore isn't reliable if it isn't.
pub fn grapheme_width(grapheme: &str) -> usize {
    if grapheme == TAB {
        return 4;
    }

    if grapheme == ZERO_WIDTH_JOINER || grapheme == VARIATION_SELECTOR_16 {
        return 0;
    }

    if grapheme.contains(ZERO_WIDTH_JOINER) {
        return 2;
    }

    for skin_tone in SKIN_TONES {
        if grapheme.contains(skin_tone) {
            return 2;
        }
    }

    grapheme.width()
}

/// Returns the display width of a string.
#[inline]
pub fn dislay_width(s: &str) -> usize {
    s.graphemes(true).map(grapheme_width).sum()
}

/// Dedents a string by removing whitespace at the start and returns the byte index of the start
/// of the dedented section, the display width of the removed segment and the dedented slice,
/// respectively.
#[inline]
pub fn dedent(s: &str) -> (usize, usize, &str) {
    let mut width = 0;
    for (index, grapheme) in s.grapheme_indices(true) {
        match grapheme {
            " " => width += 1,
            TAB => width += 4,
            _ => return (index, width, &s[index..]),
        }
    }

    (s.len(), dislay_width(s), &s[s.len()..])
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_dedent() {
        assert_eq!(dedent("  dedent this"), (2, 2, "dedent this"));
        assert_eq!(dedent("\tdedent this"), (1, 4, "dedent this"));
        assert_eq!(dedent("\t dedent this"), (2, 5, "dedent this"));
        assert_eq!(
            dedent(" \t   \t \t dedent this"),
            (9, 1 + 4 + 3 + 4 + 1 + 4 + 1, "dedent this")
        );
        assert_eq!(dedent(""), (0, 0, ""));
        assert_eq!(dedent(" "), (1, 1, ""));
        assert_eq!(dedent(" \t"), (2, 5, ""));
    }
}
