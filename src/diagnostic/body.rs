mod builder;
mod writer;

use crate::{source::SourceLine, Config, Label, Source};

/// A chunk of a diagnostic's body.
#[derive(Debug)]
pub(super) struct BodyChunk<'src> {
    pub line: SourceLine<'src>,
    pub singleline_labels: Vec<Label>,
    pub starting_multiline_labels: Vec<Label>,
    pub finishing_multiline_labels: Vec<usize>,
}

/// Describes a body and contains some cached useful information about it.
#[derive(Debug)]
pub(super) struct BodyDescriptor<'src> {
    /// The chunks of this body.
    pub chunks: Vec<BodyChunk<'src>>,
    /// How much indentation can be trimmed off in every line.
    pub indent_trim: usize,
    /// The width needed to display all line numbers in the body.
    pub line_number_width: usize,
    /// The maximum number of parallel labels that happen in the body.
    pub maximum_parallel_labels: usize,
}

impl<'src> BodyDescriptor<'src> {
    /// Builds a new [`BodyDescriptor`].
    pub(super) fn new(source: Source<'src>, labels: Vec<Label>) -> Self {
        builder::DescriptorBuilder::new(source, labels).build()
    }

    /// Writes the body described by this descriptor to a given writer.
    pub(super) fn write_to<W>(self, writer: &mut W, config: &Config) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        writer::BodyWriter::new(writer, config.clone(), self).write()
    }
}
