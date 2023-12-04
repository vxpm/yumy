use nonmax::NonMaxU32;
use owo_colors::Style;

/// Unit struct that represents the absence of
/// a source in a diagnostic.
#[derive(Debug, Clone, Copy)]
pub struct NoSource;

/// A span into the source of a [`crate::Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    start: NonMaxU32,
    end: u32,
}

impl SourceSpan {
    /// Creates a new source span. `start` and `end`
    /// are byte indexes into the source.
    pub fn new(start: u32, end: u32) -> Self {
        assert!(end >= start);
        Self {
            start: NonMaxU32::new(start).expect("Start is non-max"),
            end,
        }
    }

    /// The start of this span. Inclusive.
    #[inline]
    pub fn start(&self) -> u32 {
        self.start.get()
    }

    /// The end of this span. Exclusive.
    #[inline]
    pub fn end(&self) -> u32 {
        self.end
    }

    /// The length of this span.
    #[inline]
    pub fn len(&self) -> u32 {
        self.end - self.start()
    }

    /// Does this span contain the given value?
    #[inline]
    pub fn contains(&self, value: u32) -> bool {
        (self.start() <= value) && (value < self.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceLine<'src> {
    pub span: SourceSpan,
    pub line: &'src str,
}

impl<'src> SourceLine<'src> {
    pub fn new(line: &'src str, span: SourceSpan) -> Self {
        Self { span, line }
    }
}

/// A source of text to use with a diagnostic.
#[derive(Debug, Clone)]
pub struct Source<'src> {
    src: &'src str,
    name: Option<&'src str>,
    style: Option<Style>,
    lines: Vec<SourceLine<'src>>,
}

impl<'src> Source<'src> {
    fn lines_of(src: &str) -> Vec<SourceLine> {
        let base_addr = src.as_ptr();
        let lines = src.lines().map(|line| {
            let line_addr = line.as_ptr();
            let offset = (line_addr as usize)
                .checked_sub(base_addr as usize)
                .expect("line should always have higher address");
            let end = offset + line.len();

            SourceLine {
                span: SourceSpan::new(offset as u32, end as u32),
                line,
            }
        });

        lines.collect()
    }

    /// Creates a new source.
    pub fn new(src: &'src str, name: Option<&'src str>) -> Self {
        Self {
            src,
            name,
            style: None,
            lines: Self::lines_of(src),
        }
    }

    /// Creates a new source with the given style.
    pub fn styled(src: &'src str, name: Option<&'src str>, style: Style) -> Self {
        Self {
            src,
            name,
            style: Some(style),
            lines: Self::lines_of(src),
        }
    }

    pub fn src(&self) -> &'src str {
        self.src
    }

    pub fn name(&self) -> Option<&'src str> {
        self.name
    }

    pub fn style(&self) -> Option<Style> {
        self.style
    }

    pub fn line(&self, index: u32) -> Option<SourceLine<'src>> {
        self.lines.get(index as usize).copied()
    }

    pub fn lines(&self) -> impl Iterator<Item = &SourceLine> {
        self.lines.iter()
    }

    pub(crate) fn line_index_of_byte(&self, byte_index: u32) -> Option<u32> {
        if byte_index > self.src.len() as u32 {
            return None;
        }

        // TODO: change to a binary search
        for (index, line) in self.lines.iter().enumerate() {
            if line.span.contains(byte_index) {
                return Some(index as u32);
            }

            if line.span.start() > byte_index {
                return Some((index - 1) as u32);
            }
        }

        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SAMPLE: &str = include_str!("../sample.txt");

    #[test]
    fn test_lines() {
        let src = Source::new(SAMPLE, None);
        let mut lines = src.lines();

        assert_eq!(
            Some(&SourceLine {
                span: SourceSpan::new(0, 20),
                line: "hello there darling!"
            }),
            lines.next()
        );

        // notice how index 20 is not included in any line
        // since it is the \n character!

        assert_eq!(
            Some(&SourceLine {
                span: SourceSpan::new(21, 28),
                line: "this is"
            }),
            lines.next()
        );

        // the same goes for 28

        assert_eq!(
            Some(&SourceLine {
                span: SourceSpan::new(29, 29),
                line: ""
            }),
            lines.next()
        );

        // index 29 is the \n! but it is also not included since
        // the line is empty (29..29)

        assert_eq!(
            Some(&SourceLine {
                span: SourceSpan::new(30, 46),
                line: "a sample text :)"
            }),
            lines.next()
        );
    }
}
