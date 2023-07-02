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
            start: NonMaxU32::new(start).unwrap(),
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceLine<'src> {
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
    fn lines(src: &str) -> Vec<SourceLine> {
        let line_starts = std::iter::once(0).chain(src.match_indices('\n').map(|(i, _)| i + 1));
        let line_spans =
            line_starts
                .zip(src.lines().map(|x| (x, x.len())))
                .map(|(start, (line, len))| {
                    (line, SourceSpan::new(start as u32, (start + len) as u32))
                });

        line_spans
            .map(|(line, span)| SourceLine::new(line, span))
            .collect()
    }

    /// Creates a new source.
    pub fn new(src: &'src str, name: Option<&'src str>) -> Self {
        Self {
            src,
            name,
            style: None,
            lines: Self::lines(src),
        }
    }

    /// Creates a new source with the given style.
    pub fn styled(src: &'src str, name: Option<&'src str>, style: Style) -> Self {
        Self {
            src,
            name,
            style: Some(style),
            lines: Self::lines(src),
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

    pub(crate) fn line(&self, index: u32) -> Option<SourceLine<'src>> {
        self.lines.get(index as usize).copied()
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
        }

        None
    }
}
