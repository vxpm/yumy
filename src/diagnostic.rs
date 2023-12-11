mod body;

/// Module for diagnostic configuration related items.
pub mod config;

use self::config::Config;
use super::source::{NoSource, Source, SourceSpan};
use body::BodyWriter;
use owo_colors::{OwoColorize, Style};
use std::{
    io::{BufWriter, Write},
    ops::Range,
};

/// A label is a message that points to a specific
/// part of the source of a [`Diagnostic`].
#[derive(Debug, Clone)]
pub struct Label {
    /// The message of this label.
    pub message: String,
    /// The span this label refers to.
    pub span: SourceSpan,
    /// The indicator style of this label.
    pub indicator_style: Option<Style>,
}

impl Label {
    /// Creates a new label.
    pub fn new<S, M>(span: S, message: M) -> Self
    where
        S: Into<SourceSpan>,
        M: ToString,
    {
        Self {
            message: message.to_string(),
            span: span.into(),
            indicator_style: None,
        }
    }

    /// Creates a new label with the given style for it's indicator.
    pub fn styled<S, M>(span: S, message: M, style: Style) -> Self
    where
        S: Into<SourceSpan>,
        M: ToString,
    {
        Self {
            message: message.to_string(),
            span: span.into(),
            indicator_style: Some(style),
        }
    }

    /// Returns the line range of this label in the given source.
    ///
    /// # Panics
    /// Panics if the span is out of bounds.
    fn line_range(&self, src: &Source) -> Range<u32> {
        src.line_range_of_span(self.span)
            .expect("label should have span in range")
    }

    /// Returns whether this label is singleline or not.
    ///
    /// # Panics
    /// Panics if [`Label::line_range`] would panic.
    fn is_singleline(&self, src: &Source) -> bool {
        let line_range = self.line_range(src);
        line_range.start + 1 == line_range.end
    }
}

/// A diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic<Src> {
    message: String,
    labels: Vec<Label>,
    footnotes: Vec<String>,
    source: Src,
}

impl Diagnostic<NoSource> {
    /// Create a new diagnostic without an associated source.
    #[inline]
    pub fn new<M>(message: M) -> Self
    where
        M: ToString,
    {
        Self {
            message: message.to_string(),
            labels: Vec::new(),
            footnotes: Vec::new(),
            source: NoSource,
        }
    }

    /// Attach a source to this diagnostic.
    #[inline(always)]
    pub fn with_source(self, source: Source<'_>) -> Diagnostic<Source<'_>> {
        Diagnostic {
            message: self.message,
            labels: self.labels,
            footnotes: self.footnotes,
            source,
        }
    }
}

impl<Src> Diagnostic<Src> {
    /// Add a [`Label`] to this diagnostic.
    #[inline(always)]
    pub fn with_message<M>(mut self, message: M) -> Self
    where
        M: ToString,
    {
        self.message = message.to_string();
        self
    }

    /// Add a [`Label`] to this diagnostic.
    #[inline(always)]
    pub fn add_label(&mut self, label: Label) {
        self.labels.push(label);
    }

    /// Add a [`Label`] to this diagnostic.
    #[inline(always)]
    pub fn with_label(mut self, label: Label) -> Self {
        self.add_label(label);
        self
    }

    /// Replaces the [`Label`]s of this diagnostic.
    #[inline(always)]
    pub fn with_labels(mut self, labels: Vec<Label>) -> Self {
        self.labels = labels;
        self
    }

    /// Add a footnote to this diagnostic. A footnote is a message
    /// shown after the body of a diagnostic.
    #[inline(always)]
    pub fn add_footnote<F>(&mut self, footnote: F)
    where
        F: ToString,
    {
        self.footnotes.push(footnote.to_string());
    }

    /// Add a footnote to this diagnostic. A footnote is a message
    /// shown after the body of a diagnostic.
    #[inline(always)]
    pub fn with_footnote<F>(mut self, footnote: F) -> Self
    where
        F: ToString,
    {
        self.add_footnote(footnote.to_string());
        self
    }
}

impl<'src> Diagnostic<Source<'src>> {
    /// Calculates the left padding necessary for this diagnostic.
    fn left_padding(&self) -> usize {
        let mut padding = 0;
        for label in &self.labels {
            // find last line of label
            let line_index = self
                .source
                .line_index_at(label.span.end().saturating_sub(1));
            let index_algs = line_index
                .map(|x| f32::log10(x as f32).floor() as usize)
                .unwrap_or(0);
            if index_algs > padding {
                padding = index_algs;
            }
        }

        padding + 1
    }

    fn write_header<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        writeln!(writer, "{}", self.message)?;

        let left_padding = self.left_padding();
        write!(writer, "{:padding$}", "", padding = left_padding)?;
        writeln!(
            writer,
            " {} {}{}{}",
            '@'.style(config.styles.left_column),
            '['.style(config.styles.left_column),
            self.source
                .name()
                .unwrap_or("unknown")
                .style(config.styles.source_name),
            ']'.style(config.styles.left_column)
        )?;
        Ok(())
    }

    fn write_header_compact<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        writeln!(writer, "{}", self.message)?;
        writeln!(
            writer,
            "{} {}{}{}",
            '@'.style(config.styles.left_column),
            '['.style(config.styles.left_column),
            self.source
                .name()
                .unwrap_or("unknown")
                .style(config.styles.source_name),
            "]:".style(config.styles.left_column)
        )?;
        Ok(())
    }

    fn write_body<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        let body_lines = BodyWriter::new(
            writer,
            self.source.clone(),
            config,
            self.left_padding(),
            self.labels.as_slice(),
        );

        body_lines.write()?;

        Ok(())
    }

    fn write_body_compact<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        for label in &self.labels {
            let range = label.line_range(&self.source);
            if range.start + 1 == range.end {
                writeln!(
                    writer,
                    "{} {}{}{}{}{}",
                    config.charset.vertical_bar.style(config.styles.left_column),
                    '['.style(config.styles.left_column),
                    "line ".style(config.styles.source),
                    range.start.style(config.styles.source),
                    "]: ".style(config.styles.left_column),
                    label.message
                )?;
            } else {
                writeln!(
                    writer,
                    "{} {}{}{:?}{}{}",
                    config.charset.vertical_bar.style(config.styles.left_column),
                    '['.style(config.styles.left_column),
                    "lines ".style(config.styles.source),
                    range.style(config.styles.source),
                    "]: ".style(config.styles.left_column),
                    label.message
                )?;
            }
        }

        Ok(())
    }

    fn write_footnotes<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        let left_padding = self.left_padding();

        for footnote in &self.footnotes {
            write!(
                writer,
                "{:padding$} {} ",
                "",
                '>'.style(config.styles.footnote_indicator),
                padding = left_padding
            )?;
            writeln!(writer, "{}", footnote)?;
        }

        Ok(())
    }

    fn write_footnotes_compact<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        for footnote in &self.footnotes {
            write!(writer, "{} ", '>'.style(config.styles.footnote_indicator))?;
            writeln!(writer, "{}", footnote)?;
        }

        Ok(())
    }

    /// Writes this diagnostic to the given [`Write`]r using the specified [`Config`].
    pub fn write_to<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        self.write_header(writer, config)?;
        self.write_body(writer, config)?;
        self.write_footnotes(writer, config)?;

        writeln!(writer)?;
        Ok(())
    }

    /// Writes this diagnostic to the given [`Write`]r using the specified [`Config`]
    /// in compact mode.
    pub fn write_to_compact<W>(&self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: Write,
    {
        self.write_header_compact(writer, config)?;
        self.write_body_compact(writer, config)?;
        self.write_footnotes_compact(writer, config)?;

        writeln!(writer)?;
        Ok(())
    }

    /// Writes this diagnostic to `stderr` using the specified [`Config`].
    #[inline]
    pub fn eprint(&self, config: &Config) -> std::io::Result<()> {
        let mut eout = BufWriter::new(std::io::stderr());
        self.write_to(&mut eout, config)?;
        Ok(())
    }

    /// Writes this diagnostic to `stderr` using the specified [`Config`]
    /// in compact mode.
    #[inline]
    pub fn eprint_compact(&self, config: &Config) -> std::io::Result<()> {
        let mut eout = BufWriter::new(std::io::stderr());
        self.write_to_compact(&mut eout, config)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::diagnostic_snapshot;

    #[test]
    fn test_singleline() {
        const SAMPLE: &str = include_str!("../samples/sample2.rs");
        let src = Source::new(SAMPLE, Some("src/lib.rs"));
        let diagnostic = Diagnostic::new("error[E0072]: recursive type `List` has infinite size")
            .with_label(Label::new(53..66u32, ""))
            .with_label(Label::new(83..87u32, "recursive without indirection"))
            .with_footnote("error: could not compile `playground` (lib) due to previous error")
            .with_source(src);

        diagnostic_snapshot!(diagnostic);
    }
}
