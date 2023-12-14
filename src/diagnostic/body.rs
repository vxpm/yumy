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

/// Struct that takes care of emitting the body of a diagnostic.
/// Keeping the state for this in it's own struct is easier.
pub(crate) struct BodyWriter<'src, W> {
    writer: W,
    source: Source<'src>,
    config: Config,
    left_padding: usize,
    labels: Vec<Label>,
    slots: Vec<Slot>,
    current_line: u32,
}

impl<'src, W> BodyWriter<'src, W>
where
    W: Write,
{
    /// Calculates the number of slots needed for a set
    /// of multiline labels.
    fn slots_needed(source: &Source, labels: &[Label]) -> usize {
        enum Event {
            Start(u32),
            End(u32),
        }

        impl Event {
            fn index(&self) -> u32 {
                match self {
                    Event::Start(x) => *x,
                    Event::End(x) => *x,
                }
            }
        }

        let mut events: Vec<_> = labels
            .iter()
            .flat_map(|label| {
                let line_range = label.line_range(source);
                [Event::Start(line_range.start), Event::End(line_range.end)]
            })
            .collect();

        events.sort_unstable_by(|a, b| a.index().cmp(&b.index()));

        let mut current = 0;
        let mut max = 0;
        for event in events {
            match event {
                Event::Start(_) => current += 1,
                Event::End(_) => current -= 1,
            }

            if current > max {
                max = current;
            }
        }

        max
    }

    pub(crate) fn new(
        writer: W,
        source: Source<'src>,
        config: &Config,
        left_padding: usize,
        labels: Vec<Label>,
    ) -> Self {
        Self {
            writer,
            source,
            config: config.clone(),
            left_padding,
            labels,
            slots: vec![Slot::Inactive; Self::slots_needed(&source, &labels)],
            current_line: 0,
        }
    }
}
