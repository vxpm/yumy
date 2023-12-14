use super::{config::Config, Label};
use crate::{
    source::{Source, SourceLine},
    SourceSpan,
};
use either::Either;
use owo_colors::OwoColorize;
use std::io::Write;

#[derive(Debug, Clone, Default)]
enum Slot {
    RecentlyAdded(Label),
    Active(Label),
    #[default]
    Inactive,
}

impl Slot {
    pub fn is_active(&self) -> bool {
        match self {
            Slot::RecentlyAdded(_) => true,
            Slot::Active(_) => true,
            Slot::Inactive => false,
        }
    }

    pub fn unwrap_label(self) -> Label {
        match self {
            Slot::RecentlyAdded(label) => label,
            Slot::Active(label) => label,
            Slot::Inactive => panic!("tried to call unwrap_label on inactive slot"),
        }
    }
}

#[derive(Debug, Clone)]
enum BodyAction {
    Line(u32),
    SinglelineLabel(Label),
    StartMultilineLabel { label: Label, id: u32 },
    EndMultilineLabel(u32),
}

/// Struct that takes care of emitting the body of a diagnostic.
/// Keeping the state for this in it's own struct is easier.
pub(crate) struct BodyPreprocessor<'src> {
    source: Source<'src>,
    labels: Vec<Label>,
    active_labels: Vec<(Label, u32)>,
    multiline_id: u32,
    current_line: u32,
    result: Vec<BodyAction>,
}

impl<'src> BodyPreprocessor<'src> {
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
                .line_index_at(label.span.start())
                .expect("valid label");

            if label_start_line == self.current_line {
                if label.is_singleline(&self.source) {
                    self.result.push(BodyAction::SinglelineLabel(label));
                } else {
                    self.result.push(BodyAction::StartMultilineLabel {
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
        self.active_labels.retain(|(label, label_id)| {
            let finished = self
                .source
                .line_index_at(label.span.end())
                .expect("valid label")
                != self.current_line;

            if finished {
                self.result.push(BodyAction::EndMultilineLabel(*label_id));
            }

            finished
        });
    }

    pub(crate) fn preprocess(mut self) -> Vec<BodyAction> {
        // here's how it should go:
        // - if no active labels:
        // -- find next label and jump to its start
        // -- if singleline, emit the label
        // -- if multiline, start it
        // - if active labels:
        // -- go line by line
        // -- if a singleline label is in its start, emit it
        // -- if a multiline ends, remove it

        while !self.labels.is_empty() {
            if self.active_labels.is_empty() {
                let label = self.labels.last().expect("has remaining labels");
                self.current_line = self
                    .source
                    .line_index_at(label.span.start())
                    .expect("label span is valid");
            } else {
                self.current_line += 1;
            }

            self.result.push(BodyAction::Line(self.current_line));
            self.emit_labels_in_current();
            self.finish_labels_in_current();
        }

        self.result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Label, Source};

    #[test]
    fn test_preprocess_singleline() {
        let src = Source::new(crate::test::RUST_SAMPLE, Some("src/lib.rs"));
        let labels = vec![
            Label::new(53..66u32, ""),
            Label::new(83..87u32, "recursive without indirection"),
        ];

        let preprocessor = BodyPreprocessor::new(src, labels);

        // TODO: make this go to the correct directory!!
        insta::assert_debug_snapshot!(preprocessor.preprocess());
    }
}
