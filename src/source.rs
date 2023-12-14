use std::{ops::Range, sync::Arc};

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

    /// Whether this span is empty or not.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start() == self.end
    }

    /// Does this span contain the given value?
    #[inline]
    pub fn contains(&self, value: u32) -> bool {
        (self.start() <= value) && (value < self.end)
    }
}

impl<T> From<Range<T>> for SourceSpan
where
    T: Into<u32>,
{
    fn from(value: Range<T>) -> Self {
        Self::new(value.start.into(), value.end.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceLine<'src> {
    pub index: usize,
    pub span: SourceSpan,
    pub text: &'src str,
}

impl<'src> SourceLine<'src> {
    pub fn new(index: usize, span: SourceSpan, text: &'src str) -> Self {
        Self { index, span, text }
    }
}

/// A source of text to use with a diagnostic.
#[derive(Debug, Clone)]
pub struct Source<'src> {
    src: &'src str,
    name: Option<&'src str>,
    style: Option<Style>,
    lines: Arc<Vec<SourceLine<'src>>>,
}

impl<'src> Source<'src> {
    fn lines_of(src: &str) -> Vec<SourceLine> {
        let base_addr = src.as_ptr();
        let lines = src.lines().enumerate().map(|(index, line)| {
            let line_addr = line.as_ptr();
            let offset = (line_addr as usize)
                .checked_sub(base_addr as usize)
                .expect("line should always have higher address");
            let end = offset + line.len();

            SourceLine {
                index,
                span: SourceSpan::new(offset as u32, end as u32),
                text: line,
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
            lines: Arc::new(Self::lines_of(src)),
        }
    }

    /// Creates a new source with the given style.
    pub fn styled(src: &'src str, name: Option<&'src str>, style: Style) -> Self {
        Self {
            src,
            name,
            style: Some(style),
            lines: Arc::new(Self::lines_of(src)),
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

    pub fn line(&self, index: usize) -> Option<SourceLine<'src>> {
        self.lines.get(index as usize).copied()
    }

    pub fn lines(&self) -> impl Iterator<Item = &SourceLine> {
        self.lines.iter()
    }

    pub(crate) fn line_index_at(&self, index: usize) -> Option<usize> {
        if index > self.src.len() {
            return None;
        }

        self.lines
            .partition_point(|line| line.span.start() as usize <= index)
            .checked_sub(1)
            .map(|x| x)
    }

    /// Returns the line range of a span in this source.
    pub fn line_range_of_span(&self, span: SourceSpan) -> Option<Range<usize>> {
        let start = self.line_index_at(span.start() as usize)?;
        let end = self.line_index_at(span.end().saturating_sub(1).max(span.start()) as usize)?;

        Some(start..end + 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SAMPLE: &str = include_str!("../samples/sample1.txt");

    #[test]
    fn test_lines() {
        let src = Source::new(SAMPLE, None);
        let mut lines = src.lines();

        assert_eq!(
            Some(&SourceLine {
                index: 0,
                span: SourceSpan::new(0, 20),
                text: "hello there darling!"
            }),
            lines.next()
        );

        // notice how index 20 is not included in any line
        // since it is the \n character!

        assert_eq!(
            Some(&SourceLine {
                index: 1,
                span: SourceSpan::new(21, 28),
                text: "this is"
            }),
            lines.next()
        );

        // the same goes for 28

        assert_eq!(
            Some(&SourceLine {
                index: 2,
                span: SourceSpan::new(29, 29),
                text: ""
            }),
            lines.next()
        );

        // index 29 is the \n! but it is also not included since
        // the line is empty (29..29)

        assert_eq!(
            Some(&SourceLine {
                index: 3,
                span: SourceSpan::new(30, 46),
                text: "a sample text :)"
            }),
            lines.next()
        );
    }

    #[test]
    pub fn test_line_range() {
        let src = Source::new(SAMPLE, None);

        assert_eq!(Some(1..2), src.line_range_of_span(SourceSpan::new(21, 28)));
        assert_eq!(Some(1..2), src.line_range_of_span(SourceSpan::new(21, 29)));
        assert_eq!(Some(1..3), src.line_range_of_span(SourceSpan::new(21, 30)));
        assert_eq!(Some(1..4), src.line_range_of_span(SourceSpan::new(21, 31)));
    }
}
