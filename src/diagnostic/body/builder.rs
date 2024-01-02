use super::{BodyChunk, BodyDescriptor};
use crate::{source::Source, Label, SourceSpan};

/// Struct that takes care of building a descriptor.
/// Keeping the state for this in it's own struct is easier.
pub(super) struct DescriptorBuilder<'src> {
    source: Source<'src>,
    labels: Vec<Label>,
    active_labels: Vec<(usize, SourceSpan)>,
    multiline_id: usize,
    current_line: usize,
    indent_trim: usize,
    result: Vec<BodyChunk<'src>>,
}

impl<'src> DescriptorBuilder<'src> {
    /// Creates a new [`DescriptorBuilder`].
    pub(super) fn new(source: Source<'src>, mut labels: Vec<Label>) -> Self {
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

    /// Returns the ID of multiline labels finishing in the current line.
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

            let line = self
                .source
                .lines()
                .get(self.current_line)
                .copied()
                .expect("valid line");

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

    /// Calculates the width of the line number section in the body.
    fn calculate_line_number_width(&self) -> usize {
        self.result
            .last()
            .map(|chunk| (chunk.line.index() + 1).ilog10() as usize + 1)
            .unwrap_or(0)
    }

    /// Calculates the maximum number of parallel multiline labels that happens in this descriptor.
    fn calculate_maximum_parallel_labels(&self) -> usize {
        let mut count = 0;
        let mut max = 0;
        for chunk in self.result.iter() {
            count += chunk.starting_multiline_labels.len();
            max = max.max(count);

            // NOTE: this is >after< we recalculate the maximum because labels that finish on a
            // line are still shown on it!
            count -= chunk.finishing_multiline_labels.len();
        }

        max
    }

    /// Builds the [`BodyDescriptor`].
    pub(crate) fn build(mut self) -> BodyDescriptor<'src> {
        self.emit_events();

        let line_number_width = self.calculate_line_number_width();
        let maximum_parallel_labels = self.calculate_maximum_parallel_labels();

        BodyDescriptor {
            chunks: self.result,
            indent_trim: self.indent_trim,
            line_number_width,
            maximum_parallel_labels,
        }
    }
}

#[cfg(test)]
mod test {
    use owo_colors::Style;

    use super::*;
    use crate::{Label, Source};

    #[test]
    fn test_build_singleline() {
        let src = Source::new(crate::test::RUST_SAMPLE_1, Some("src/lib.rs"));
        let labels = vec![
            Label::new(53..66u32, ""),
            Label::new(83..87u32, "recursive without indirection"),
        ];

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(BodyDescriptor::new(src, labels));
    }

    #[test]
    fn test_build_multiline_1() {
        let src = Source::new(crate::test::RUST_SAMPLE_2, Some("src/main.rs"));
        let labels = vec![
            Label::styled(
                247..260u32,
                "required by a bound introduced by this call",
                Style::new().yellow(),
            ),
            Label::styled(
                261..357u32,
                "`Rc<Mutex<i32>>` cannot be sent between threads safely",
                Style::new().red(),
            ),
        ];

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(BodyDescriptor::new(src, labels));
    }

    #[test]
    fn test_build_multiline_2() {
        let src = Source::new(crate::test::TEXT_SAMPLE_2, Some("just testing"));
        let labels = vec![
            Label::new(0..36u32, "just testing two multilines"),
            Label::new(10..24u32, "hi"),
            Label::new(28u32..35u32, "hello"),
        ];

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(BodyDescriptor::new(src, labels));
    }
}
