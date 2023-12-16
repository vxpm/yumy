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

/// A line of a text source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceLine<'src> {
    index: usize,
    indent_size: usize,
    full_span: SourceSpan,
    dedented_span: SourceSpan,
    text: &'src str,
}

impl<'src> SourceLine<'src> {
    /// The index of this line in the source.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The size of the indentation in this line.
    pub fn indent_size(&self) -> usize {
        self.indent_size
    }

    /// The span of the whole line in the source, i.e. including indentation.
    pub fn full_span(&self) -> SourceSpan {
        self.full_span
    }

    /// The span of the dedented line in the source, i.e. excluding indentation.
    pub fn dedented_span(&self) -> SourceSpan {
        self.dedented_span
    }

    /// The dedented text of this line.
    pub fn text(&self) -> &'src str {
        self.text
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
            let (dedented_offset, indent_size, dedented) = crate::text::dedent(line);

            SourceLine {
                index,
                indent_size,
                full_span: SourceSpan::new(offset as u32, end as u32),
                dedented_span: SourceSpan::new((offset + dedented_offset) as u32, end as u32),
                text: dedented,
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
            .partition_point(|line| line.full_span.start() as usize <= index)
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
                indent_size: 0,
                full_span: SourceSpan::new(0, 20),
                dedented_span: SourceSpan::new(0, 20),
                text: "hello there darling!",
            }),
            lines.next()
        );

        // notice how index 20 is not included in any line
        // since it is the \n character!

        assert_eq!(
            Some(&SourceLine {
                index: 1,
                indent_size: 0,
                full_span: SourceSpan::new(21, 28),
                dedented_span: SourceSpan::new(21, 28),
                text: "this is"
            }),
            lines.next()
        );

        // the same goes for 28

        assert_eq!(
            Some(&SourceLine {
                index: 2,
                indent_size: 0,
                full_span: SourceSpan::new(29, 29),
                dedented_span: SourceSpan::new(29, 29),
                text: ""
            }),
            lines.next()
        );

        // index 29 is the \n! but it is also not included since
        // the line is empty (29..29)

        assert_eq!(
            Some(&SourceLine {
                index: 3,
                indent_size: 0,
                full_span: SourceSpan::new(30, 46),
                dedented_span: SourceSpan::new(30, 46),
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
