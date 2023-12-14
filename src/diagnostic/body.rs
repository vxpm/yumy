use super::Label;
use crate::{
    source::{Source, SourceLine},
    Config, SourceSpan,
};
use owo_colors::OwoColorize;
use std::io::Write;

#[derive(Debug, Clone)]
pub(crate) enum BodyEvent<'src> {
    EmitLine(SourceLine<'src>),
    EmitSinglelineLabel(Label),
    StartMultilineLabel { label: Label, id: usize },
    EndMultilineLabel(usize),
}

/// Struct that takes care of generating the body of a diagnostic.
/// Keeping the state for this in it's own struct is easier.
pub(crate) struct BodyBuilder<'src> {
    source: Source<'src>,
    labels: Vec<Label>,
    active_labels: Vec<(usize, SourceSpan)>,
    multiline_id: usize,
    current_line: usize,
    result: Vec<BodyEvent<'src>>,
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
            result: Vec::new(),
        }
    }

    fn emit_labels_in_current(&mut self) {
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
                    self.result.push(BodyEvent::EmitSinglelineLabel(label));
                } else {
                    self.active_labels.push((self.multiline_id, label.span));
                    self.result.push(BodyEvent::StartMultilineLabel {
                        label: label,
                        id: self.multiline_id,
                    });
                    self.multiline_id += 1;
                }
            } else {
                self.labels.push(label);
                break;
            }
        }
    }

    fn finish_labels_in_current(&mut self) {
        self.active_labels.retain(|(label_id, span)| {
            let finished = self
                .source
                .line_index_at(span.end() as usize)
                .expect("valid label")
                == self.current_line;

            if finished {
                self.result.push(BodyEvent::EndMultilineLabel(*label_id));
            }

            !finished
        });
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
            self.result.push(BodyEvent::EmitLine(line));
            self.emit_labels_in_current();
            self.finish_labels_in_current();
        }
    }

    fn preprocess(&mut self) {
        // todo
    }

    pub(crate) fn build(mut self) -> BodyDescriptor<'src> {
        self.emit_events();
        self.preprocess();
        BodyDescriptor(self.result)
    }
}

#[derive(Debug)]
pub(crate) struct BodyDescriptor<'src>(Vec<BodyEvent<'src>>);

impl<'src> BodyDescriptor<'src> {
    /// Calculates the maximum number of parallel multiline labels that happens in this descriptor.
    fn maximum_parallel_labels(&self) -> usize {
        let mut count = 0;
        let mut max = 0;
        for event in self.0.iter() {
            match event {
                BodyEvent::StartMultilineLabel { .. } => count += 1,
                BodyEvent::EndMultilineLabel(_) => count -= 1,
                _ => (),
            }

            max = max.max(count);
        }

        max
    }

    /// Calculates the width of the line number section in the body.
    fn line_number_width(&self) -> usize {
        self.0
            .iter()
            .rev()
            .find_map(|event| {
                if let BodyEvent::EmitLine(line) = event {
                    Some(line.index + 1)
                } else {
                    None
                }
            })
            .map(|line_index| f32::log10(line_index as f32).floor() as usize)
            .unwrap_or(0)
    }
}

pub(crate) struct BodyWriter<'src, W> {
    writer: W,
    config: Config,
    descriptor: BodyDescriptor<'src>,
    slots: Vec<Option<Label>>,
    line_number_width: usize,
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
            slots: vec![None; slots_needed],
            line_number_width,
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

    pub(crate) fn write(mut self) -> std::io::Result<()> {
        let events = std::mem::take(&mut self.descriptor.0);
        for event in events {
            match event {
                BodyEvent::EmitLine(line) => {
                    self.emit_left_column(Some(line.index + 1))?;
                    writeln!(self.writer, "{}", line.text)?;
                }
                BodyEvent::EmitSinglelineLabel(_) => (),
                BodyEvent::StartMultilineLabel { label, id } => (),
                BodyEvent::EndMultilineLabel(_) => (),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Label, Source};

    #[test]
    fn test_preprocess_singleline() {
        let src = Source::new(crate::test::RUST_SAMPLE_1, Some("src/lib.rs"));
        let labels = vec![
            Label::new(53..66u32, ""),
            Label::new(83..87u32, "recursive without indirection"),
        ];

        let preprocessor = BodyBuilder::new(src, labels);

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(preprocessor.build());
    }

    #[test]
    fn test_preprocess_multiline() {
        let src = Source::new(crate::test::RUST_SAMPLE_2, Some("src/main.rs"));
        let labels = vec![
            Label::new(247..260u32, "required by a bound introduced by this call"),
            Label::new(
                261..357u32,
                "`Rc<Mutex<i32>>` cannot be sent between threads safely",
            ),
        ];

        let preprocessor = BodyBuilder::new(src, labels);

        crate::test::setup_insta!();
        insta::assert_debug_snapshot!(preprocessor.build());
    }
}
