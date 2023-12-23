use super::Label;
use crate::{
    source::{Source, SourceLine},
    Config, SourceSpan,
};
use owo_colors::OwoColorize;
use std::io::Write;

#[derive(Debug)]
pub(crate) struct BodyChunk<'src> {
    line: SourceLine<'src>,
    singleline_labels: Vec<Label>,
    starting_multiline_labels: Vec<Label>,
    finishing_multiline_labels: Vec<usize>,
}

/// Struct that takes care of generating the body of a diagnostic.
/// Keeping the state for this in it's own struct is easier.
pub(crate) struct BodyBuilder<'src> {
    source: Source<'src>,
    labels: Vec<Label>,
    active_labels: Vec<(usize, SourceSpan)>,
    multiline_id: usize,
    current_line: usize,
    indent_trim: usize,
    result: Vec<BodyChunk<'src>>,
}

impl<'src> BodyBuilder<'src> {
    pub(crate) fn new(source: Source<'src>, mut labels: Vec<Label>) -> Self {
        // sort labels by their start, in reverse order (make it a stack)
        labels.sort_by_key(|label| std::cmp::Reverse(label.span.start()));
        Self {
            source,
            labels,
            active_labels: Vec::new(),
            multiline_id: 0,
            current_line: 0,
            indent_trim: usize::MAX,
            result: Vec::new(),
        }
    }

    /// Returns the singleline labels of the current line and the multiline labels starting in the
    /// current line, respectively.
    fn emit_labels_in_current(&mut self) -> (Vec<Label>, Vec<Label>) {
        let mut singleline_labels = Vec::new();
        let mut multiline_labels = Vec::new();
        loop {
            let Some(label) = self.labels.pop() else {
                break;
            };

            let label_start_line = self
                .source
                .line_index_at(label.span.start() as usize)
                .expect("valid label");

            if label_start_line == self.current_line {
                if label.is_singleline(&self.source) {
                    singleline_labels.push(label);
                } else {
                    self.active_labels.push((self.multiline_id, label.span));
                    multiline_labels.push(label);
                    self.multiline_id += 1;
                }
            } else {
                // put the label back
                self.labels.push(label);
                break;
            }
        }

        (singleline_labels, multiline_labels)
    }

    /// Returns the ID of multiline labels finishing in the current line
    fn finish_labels_in_current(&mut self) -> Vec<usize> {
        let mut finished_multiline_labels = Vec::new();
        self.active_labels.retain(|(label_id, span)| {
            let finished = self
                .source
                .line_index_at(span.end() as usize)
                .expect("valid label")
                == self.current_line;

            if finished {
                finished_multiline_labels.push(*label_id);
            }

            !finished
        });

        finished_multiline_labels
    }

    fn emit_events(&mut self) {
        // here's how it should go:
        // - if no active labels:
        // -- find next label and jump to its start
        // -- if singleline, emit the label
        // -- if multiline, start it
        // - if active labels:
        // -- go line by line
        // -- if a singleline label is in its start, emit it
        // -- if a multiline ends, remove it

        while !self.labels.is_empty() || !self.active_labels.is_empty() {
            if self.active_labels.is_empty() {
                let label = self.labels.last().expect("has remaining labels");
                self.current_line = self
                    .source
                    .line_index_at(label.span.start() as usize)
                    .expect("label span is valid");
            } else {
                self.current_line += 1;
            }

            let line = self.source.line(self.current_line).expect("valid line");

            // special case: if the line is empty, don't consider it for indent trimming
            if !line.text().is_empty() {
                self.indent_trim = self.indent_trim.min(line.indent_size());
            }

            let (singleline_labels, starting_multiline_labels) = self.emit_labels_in_current();
            let finishing_multiline_labels = self.finish_labels_in_current();
            let chunk = BodyChunk {
                line,
                singleline_labels,
                starting_multiline_labels,
                finishing_multiline_labels,
            };
            self.result.push(chunk);
        }
    }

    pub(crate) fn build(mut self) -> BodyDescriptor<'src> {
        self.emit_events();
        BodyDescriptor {
            chunks: self.result,
            indent_trim: self.indent_trim,
        }
    }
}

#[derive(Debug)]
pub(crate) struct BodyDescriptor<'src> {
    chunks: Vec<BodyChunk<'src>>,
    indent_trim: usize,
}

impl<'src> BodyDescriptor<'src> {
    /// Calculates the maximum number of parallel multiline labels that happens in this descriptor.
    fn maximum_parallel_labels(&self) -> usize {
        let mut count = 0;
        let mut max = 0;
        for chunk in self.chunks.iter() {
            count += chunk.starting_multiline_labels.len();
            max = max.max(count);

            // NOTE: this is >after< we recalculate the maximum because labels that finish on a
            // line are still shown on it!
            count -= chunk.finishing_multiline_labels.len();
        }

        max
    }

    /// Calculates the width of the line number section in the body.
    fn line_number_width(&self) -> usize {
        self.chunks
            .last()
            .map(|chunk| (chunk.line.index() + 1).ilog10() as usize + 1)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default)]
enum Slot {
    RecentlyAdded(usize, Label),
    Active(usize, Label),
    #[default]
    Inactive,
}

impl Slot {
    pub fn is_active(&self) -> bool {
        match self {
            Slot::RecentlyAdded(_, _) => true,
            Slot::Active(_, _) => true,
            Slot::Inactive => false,
        }
    }
}

pub(crate) struct BodyWriter<'src, W> {
    writer: W,
    config: Config,
    descriptor: BodyDescriptor<'src>,
    slots: Vec<Slot>,
    multiline_id: usize,
    line_number_width: usize,
    current_indent_level: usize,
}

impl<'src, W> BodyWriter<'src, W>
where
    W: Write,
{
    pub(crate) fn new(writer: W, config: Config, descriptor: BodyDescriptor<'src>) -> Self {
        let slots_needed = descriptor.maximum_parallel_labels();
        let line_number_width = descriptor.line_number_width();

        Self {
            writer,
            config,
            descriptor,
            slots: vec![Slot::Inactive; slots_needed],
            multiline_id: 0,
            line_number_width,
            current_indent_level: 0,
        }
    }

    fn emit_left_column(&mut self, line_number: Option<usize>) -> std::io::Result<()> {
        if let Some(index) = line_number {
            write!(
                self.writer,
                "{:padding$} {} ",
                (index + 1).style(self.config.styles.left_column),
                self.config
                    .charset
                    .vertical_bar
                    .style(self.config.styles.left_column),
                padding = self.line_number_width
            )?;
        } else {
            write!(
                self.writer,
                "{:padding$} {} ",
                "",
                self.config
                    .charset
                    .separator
                    .style(self.config.styles.left_column),
                padding = self.line_number_width
            )?;
        }

        Ok(())
    }

    fn emit_multiline_indicators(
        &mut self,
        finishing_multiline_labels: &[usize],
    ) -> std::io::Result<()> {
        for slot in self.slots.iter_mut() {
            let (is_new, label_id, label) = match std::mem::take(slot) {
                Slot::RecentlyAdded(label_id, label) => {
                    *slot = Slot::Active(label_id, label);
                    let Slot::Active(label_id, ref label) = slot else {
                        unreachable!()
                    };

                    (true, *label_id, label)
                }
                Slot::Active(label_id, label) => {
                    *slot = Slot::Active(label_id, label);
                    let Slot::Active(label_id, ref label) = slot else {
                        unreachable!()
                    };

                    (false, *label_id, label)
                }
                Slot::Inactive => {
                    write!(self.writer, "  ")?;
                    continue;
                }
            };

            let style = label
                .indicator_style
                .unwrap_or(self.config.styles.multiline_indicator);

            let indicator_char = if is_new {
                self.config.charset.multiline_start
            } else if finishing_multiline_labels.contains(&label_id) {
                self.config.charset.multiline_end
            } else {
                self.config.charset.vertical_bar
            };

            write!(self.writer, "{} ", indicator_char.style(style))?;
        }

        // write!(self.writer, " ",)?;
        Ok(())
    }

    fn emit_source_line(&mut self, chunk: &BodyChunk) -> std::io::Result<()> {
        self.emit_left_column(Some(chunk.line.index() + 1))?;
        self.emit_multiline_indicators(&chunk.finishing_multiline_labels)?;

        // remember the special case: if the line is empty, don't
        // attempt to trim it
        self.current_indent_level = if chunk.line.text().is_empty() {
            0
        } else {
            chunk.line.indent_size() - self.descriptor.indent_trim
        };

        // finally, write the line
        writeln!(
            self.writer,
            "{:l$}{}",
            "",
            chunk.line.text(),
            l = self.current_indent_level,
        )?;
        Ok(())
    }

    fn emit_singleline_labels(&mut self, chunk: &mut BodyChunk) -> std::io::Result<()> {
        let line = chunk.line;
        let labels = std::mem::take(&mut chunk.singleline_labels);
        for label in labels {
            self.emit_left_column(None)?;
            self.emit_multiline_indicators(&chunk.finishing_multiline_labels)?;

            // calculate ranges into the line text
            let local_base = line.dedented_span().start();
            let before_underline_range = 0usize..(label.span.start() - local_base) as usize;
            let underline_range =
                before_underline_range.end..(label.span.end() - local_base) as usize;

            // compute widths
            let before_underline_width =
                crate::text::dislay_width(&line.text()[before_underline_range]);
            let underline_width = crate::text::dislay_width(&line.text()[underline_range]);

            // write label
            let before_underline = std::iter::repeat(' ').take(before_underline_width);
            let underline = std::iter::repeat(self.config.charset.underliner).take(underline_width);
            let before_label: String = before_underline.chain(underline).collect();
            let before_label_style = label
                .indicator_style
                .unwrap_or(self.config.styles.singleline_indicator);

            writeln!(
                self.writer,
                "{:l$}{} {}",
                "",
                before_label.style(before_label_style),
                label.message,
                l = self.current_indent_level,
            )?;
        }

        Ok(())
    }

    fn allocate_multiline(&mut self, label: Label) {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| !slot.is_active())
            .expect("has enough slots");

        *slot = Slot::RecentlyAdded(self.multiline_id, label);
        self.multiline_id += 1;
    }

    fn deallocate_multiline(&mut self, label_id: usize) {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| match slot {
                Slot::RecentlyAdded(id, _) => *id == label_id,
                Slot::Active(id, _) => *id == label_id,
                Slot::Inactive => false,
            })
            .expect("is active in a slot");

        *slot = Slot::Inactive;
    }

    fn start_multiline_labels(&mut self, chunk: &mut BodyChunk) -> std::io::Result<()> {
        let labels = std::mem::take(&mut chunk.starting_multiline_labels);
        for label in labels {
            self.allocate_multiline(label);
        }

        Ok(())
    }

    fn finish_multiline_labels(&mut self, chunk: &mut BodyChunk) -> std::io::Result<()> {
        let labels = std::mem::take(&mut chunk.finishing_multiline_labels);
        for label_id in labels {
            self.deallocate_multiline(label_id);
        }

        Ok(())
    }

    pub(crate) fn write(mut self) -> std::io::Result<()> {
        let chunks = std::mem::take(&mut self.descriptor.chunks);
        for mut chunk in chunks {
            self.start_multiline_labels(&mut chunk)?;
            self.emit_source_line(&chunk)?;
            self.emit_singleline_labels(&mut chunk)?;
            self.finish_multiline_labels(&mut chunk)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Label, Source};

    #[test]
    fn test_build_singleline() {
        let src = Source::new(crate::test::RUST_SAMPLE_1, Some("src/lib.rs"));
        let labels = vec![
            Label::new(53..66u32, ""),
            Label::new(83..87u32, "recursive without indirection"),
        ];

        let builder = BodyBuilder::new(src, labels);

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(builder.build());
    }

    #[test]
    fn test_build_multiline_1() {
        let src = Source::new(crate::test::RUST_SAMPLE_2, Some("src/main.rs"));
        let labels = vec![
            Label::new(247..260u32, "required by a bound introduced by this call"),
            Label::new(
                261..357u32,
                "`Rc<Mutex<i32>>` cannot be sent between threads safely",
            ),
        ];

        let builder = BodyBuilder::new(src, labels);

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(builder.build());
    }

    #[test]
    fn test_build_multiline_2() {
        let src = Source::new(crate::test::TEXT_SAMPLE_2, Some("just testing"));
        let labels = vec![
            Label::new(0..36u32, "just testing two multilines"),
            Label::new(10..24u32, "hi"),
            Label::new(28u32..35u32, "hello"),
        ];

        let builder = BodyBuilder::new(src, labels);

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(builder.build());
    }
}
