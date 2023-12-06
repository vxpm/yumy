use super::{config::Config, Label};
use crate::{
    source::{Source, SourceLine},
    SourceSpan,
};
use either::Either;
use owo_colors::{OwoColorize, Style};
use std::{io::Write, ops::Range};

#[derive(Debug, Clone, Copy)]
struct IdentInfo {
    end: usize,
    len: usize,
}

fn ident_info(text: &str) -> IdentInfo {
    let mut len = 0;
    let mut c_indices = text.char_indices();

    let end = loop {
        let Some((start, c)) = c_indices.next() else {
            break text.len();
        };

        len += match c {
            ' ' => 1,
            '\t' => 4,
            _ => break start,
        }
    };

    IdentInfo { end, len }
}

#[derive(Debug, Clone)]
struct SinglelineLabel {
    message: String,
    line: u32,
    line_span: SourceSpan,
    indicator_style: Option<Style>,
}

#[derive(Debug, Clone)]
struct MultilineLabel {
    message: String,
    line_range: Range<u32>,
    indicator_style: Option<Style>,
}

#[derive(Debug, Clone, Default)]
enum Slot {
    RecentlyAdded(MultilineLabel),
    Active(MultilineLabel),
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

    pub fn unwrap_label(self) -> MultilineLabel {
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
    ident_len: usize,
    singleline_labels: Vec<SinglelineLabel>,
    multiline_labels: Vec<MultilineLabel>,
    multiline_slots: Vec<Slot>,
    current_line: u32,
}

impl<'src, W> BodyWriter<'src, W>
where
    W: Write,
{
    /// Calculates the number of slots needed for a set
    /// of multiline labels.
    fn slots_needed(labels: &[MultilineLabel]) -> usize {
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
            .flat_map(|x| {
                [
                    Event::Start(x.line_range.start),
                    Event::End(x.line_range.end),
                ]
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
        labels: &[Label],
    ) -> Self {
        let mut singleline_labels = Vec::new();
        let mut multiline_labels = Vec::new();
        for label in labels {
            if label.is_singleline(&source) {
                let line_index = label.line_range(&source).start;
                let line = source.line(line_index).unwrap();

                let label_line_start = label.span.start() - line.span.start();
                let label_line_end = label_line_start + label.span.len();

                let label = SinglelineLabel {
                    message: label.message.clone(),
                    line: label.line_range(&source).start,
                    line_span: SourceSpan::new(label_line_start, label_line_end),
                    indicator_style: label.indicator_style,
                };
                singleline_labels.push(label);
            } else {
                let label = MultilineLabel {
                    message: label.message.clone(),
                    line_range: label.line_range(&source),
                    indicator_style: label.indicator_style,
                };

                multiline_labels.push(label);
            }
        }

        let singleline_lines = singleline_labels
            .iter()
            .map(|label| source.line(label.line).unwrap());
        let multiline_lines = multiline_labels.iter().flat_map(|label| {
            [
                source.line(label.line_range.start).unwrap(),
                source.line(label.line_range.end - 1).unwrap(),
            ]
        });

        let ident_width = singleline_lines
            .chain(multiline_lines)
            .map(|line| ident_info(line.line).len)
            .min()
            .unwrap_or(0);

        Self {
            writer,
            source,
            config: config.clone(),
            left_padding,
            ident_len: ident_width,
            singleline_labels,
            multiline_slots: vec![Slot::Inactive; Self::slots_needed(&multiline_labels)],
            multiline_labels,
            current_line: 0,
        }
    }

    /// Returns the next label, be it single or multi line.
    fn next_label(&mut self) -> Option<Either<SinglelineLabel, MultilineLabel>> {
        let next_singleline_label = self
            .singleline_labels
            .iter()
            .map(|x| x.line)
            .enumerate()
            .min_by_key(|(_, start)| *start);

        let next_multiline_label = self
            .multiline_labels
            .iter()
            .map(|x| x.line_range.start)
            .enumerate()
            .min_by_key(|(_, start)| *start);

        let (index, is_singleline) = match (next_singleline_label, next_multiline_label) {
            (None, None) => return None,
            (Some((singleline_index, _)), None) => (singleline_index, true),
            (Some((singleline_index, singleline_start)), Some((_, multiline_start)))
                if singleline_start < multiline_start =>
            {
                (singleline_index, true)
            }
            (_, Some((multiline_index, _))) => (multiline_index, false),
        };

        Some(if is_singleline {
            Either::Left(self.singleline_labels.remove(index))
        } else {
            Either::Right(self.multiline_labels.remove(index))
        })
    }

    /// Returns the next singleline label in the current line.
    fn next_singleline_label_in_current(&mut self) -> Option<SinglelineLabel> {
        self.singleline_labels
            .iter()
            .enumerate()
            .find(|(_, label)| label.line == self.current_line)
            .map(|(index, _)| index)
            .map(|index| self.singleline_labels.remove(index))
    }

    /// Returns the next multiline label starting in the current line.
    fn next_multiline_label_in_current(&mut self) -> Option<MultilineLabel> {
        self.multiline_labels
            .iter()
            .enumerate()
            .find(|(_, label)| label.line_range.start == self.current_line)
            .map(|(index, _)| index)
            .map(|index| self.multiline_labels.remove(index))
    }

    /// Are there any active multiline labels?
    #[inline]
    fn has_active_multiline_labels(&self) -> bool {
        self.multiline_slots.iter().any(|x| x.is_active())
    }

    /// Allocate the given multiline label into an available slot.
    #[inline]
    fn allocate_multiline_label(&mut self, label: MultilineLabel) {
        let slot_index = self
            .multiline_slots
            .iter()
            .enumerate()
            .find(|(_, slot)| !slot.is_active())
            .map(|(index, _)| index);

        if let Some(slot_index) = slot_index {
            self.multiline_slots[slot_index] = Slot::RecentlyAdded(label);
        } else {
            unreachable!("should have enough slots")
        }
    }

    /// Emit the left column of the body.
    #[inline]
    fn emit_left_column(&mut self, line_index: impl Into<Option<usize>>) -> std::io::Result<()> {
        if let Some(index) = line_index.into() {
            write!(
                self.writer,
                "{:padding$} {} ",
                (index + 1).style(self.config.styles.left_column),
                self.config
                    .charset
                    .vertical_bar
                    .style(self.config.styles.left_column),
                padding = self.left_padding
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
                padding = self.left_padding
            )?;
        }

        Ok(())
    }

    fn emit_multiline_indicators(&mut self) -> std::io::Result<()> {
        for slot in self.multiline_slots.iter_mut() {
            let (is_new, label) = match slot {
                slot @ Slot::RecentlyAdded(_) => {
                    let Slot::RecentlyAdded(label) = std::mem::take(slot) else {
                        unreachable!()
                    };

                    *slot = Slot::Active(label);
                    let Slot::Active(ref label) = slot else {
                        unreachable!()
                    };

                    (true, label)
                }
                Slot::Active(label) => (false, &*label),
                Slot::Inactive => {
                    write!(self.writer, " ")?;
                    continue;
                }
            };

            let style = label
                .indicator_style
                .unwrap_or(self.config.styles.multiline_indicator);

            let indicator_char = if is_new {
                self.config.charset.multiline_start
            } else if label.line_range.end == self.current_line + 1 {
                self.config.charset.multiline_end
            } else {
                self.config.charset.vertical_bar
            };

            write!(self.writer, "{}", indicator_char.style(style))?;
        }

        Ok(())
    }

    /// Emit the given source line.
    fn emit_source_line(&mut self, line: SourceLine, line_index: u32) -> std::io::Result<()> {
        self.emit_left_column(line_index as usize)?;
        self.emit_multiline_indicators()?;

        let line_ident_info = ident_info(line.line);
        let spaces = line_ident_info.len - self.ident_len;

        let style = self.source.style().unwrap_or(self.config.styles.source);

        write!(self.writer, "{:x$} ", "", x = spaces)?;
        writeln!(
            self.writer,
            "{}",
            (&line.line[line_ident_info.end..]).style(style),
        )?;
        Ok(())
    }

    /// Emit the given singleline label.
    fn emit_singleline_label(
        &mut self,
        line: SourceLine,
        label: SinglelineLabel,
    ) -> std::io::Result<()> {
        self.emit_left_column(None)?;
        self.emit_multiline_indicators()?;

        let line_ident_info = ident_info(line.line);
        let spaces = line_ident_info.len - self.ident_len;

        let line_width = unicode_width::UnicodeWidthStr::width(line.line);

        let before_underliner_width = spaces
            + unicode_width::UnicodeWidthStr::width(&line.line[..label.line_span.start() as usize]);
        let after_underliner_width =
            unicode_width::UnicodeWidthStr::width(&line.line[label.line_span.end() as usize..]);
        let underliner_width = line_width - (before_underliner_width + after_underliner_width);

        let before = std::iter::repeat(' ').take(before_underliner_width);
        let underliner = std::iter::repeat(self.config.charset.underliner).take(underliner_width);

        let underliner = before.chain(underliner);

        let style = label
            .indicator_style
            .unwrap_or(self.config.styles.singleline_indicator);

        write!(self.writer, "{:x$}", "", x = spaces)?;
        for c in underliner {
            write!(self.writer, "{}", c.style(style))?;
        }

        writeln!(self.writer, " {}", label.message)?;
        Ok(())
    }

    /// Emits all singleline labels in the current line.
    #[inline]
    fn emit_singleline_labels_in_current(&mut self, line: SourceLine) -> std::io::Result<()> {
        while let Some(label) = self.next_singleline_label_in_current() {
            self.emit_singleline_label(line, label)?;
        }

        Ok(())
    }

    /// Emit the end of the given multiline label.
    fn emit_multiline_label_end(
        &mut self,
        line: SourceLine,
        label: MultilineLabel,
        label_slot: u32,
    ) -> std::io::Result<()> {
        self.emit_left_column(None)?;
        let line_width = unicode_width::UnicodeWidthStr::width(line.line);
        let this_style = label.indicator_style;

        for slot in &self.multiline_slots[..label_slot as usize] {
            match slot {
                Slot::RecentlyAdded(_) => {
                    unreachable!("singleline multiline labels should be impossible")
                }
                Slot::Active(label) => {
                    let style = label
                        .indicator_style
                        .unwrap_or(self.config.styles.multiline_indicator);

                    write!(
                        self.writer,
                        "{}",
                        self.config.charset.vertical_bar.style(style)
                    )?;
                }
                Slot::Inactive => {
                    write!(self.writer, " ")?;
                }
            }
        }

        write!(
            self.writer,
            "{}",
            self.config
                .charset
                .connection_top_to_right
                .style(this_style.unwrap_or(self.config.styles.multiline_indicator))
        )?;

        for slot in &self.multiline_slots[label_slot as usize + 1..] {
            match slot {
                Slot::RecentlyAdded(_) => {
                    unreachable!("singleline multiline labels should be impossible")
                }
                Slot::Active(label) => {
                    let style = this_style.unwrap_or(
                        label
                            .indicator_style
                            .unwrap_or(self.config.styles.multiline_indicator),
                    );

                    write!(
                        self.writer,
                        "{}",
                        self.config.charset.multiline_crossing.style(style)
                    )?;
                }
                Slot::Inactive => {
                    write!(
                        self.writer,
                        "{}",
                        self.config
                            .charset
                            .horizontal_bar
                            .style(this_style.unwrap_or(self.config.styles.multiline_indicator))
                    )?;
                }
            }
        }

        let underliner: std::iter::Take<std::iter::Repeat<char>> =
            std::iter::repeat(self.config.charset.horizontal_bar).take(line_width + 1);
        for c in underliner {
            write!(
                self.writer,
                "{}",
                c.style(this_style.unwrap_or(self.config.styles.multiline_indicator))
            )?;
        }

        writeln!(self.writer, " {}", label.message)?;
        Ok(())
    }

    /// Try to finish any active multiline labels that end in the current line.
    fn try_finishing_active_multiline_labels(&mut self) -> std::io::Result<()> {
        let mut multiline_slot = 0;
        while multiline_slot < self.multiline_slots.len() {
            let Slot::Active(label) = &self.multiline_slots[multiline_slot] else {
                multiline_slot += 1;
                continue;
            };

            if label.line_range.end == self.current_line + 1 {
                let label =
                    std::mem::take(&mut self.multiline_slots[multiline_slot]).unwrap_label();

                let line = self.source.line(self.current_line).unwrap();
                self.emit_multiline_label_end(line, label, multiline_slot as u32)?;
            } else {
                multiline_slot += 1;
            }
        }

        Ok(())
    }

    /// Allocate all multiline labels that start in the current line.
    #[inline]
    fn start_new_multiline_labels(&mut self) {
        while let Some(label) = self.next_multiline_label_in_current() {
            self.allocate_multiline_label(label);
        }
    }

    pub(crate) fn write(mut self) -> std::io::Result<()> {
        // sort singleline labels from biggest to smallest
        self.singleline_labels
            .sort_unstable_by_key(|x| std::cmp::Reverse(x.line_span.len()));

        // sort multiline labels from bottom to top (relative to the end)
        self.multiline_labels
            .sort_unstable_by_key(|x| std::cmp::Reverse(x.line_range.end));

        loop {
            if !self.has_active_multiline_labels() {
                let Some(label) = self.next_label() else {
                    break;
                };

                match label {
                    Either::Left(label) => {
                        let line_index = label.line;
                        self.current_line = line_index;

                        let line = self.source.line(line_index).unwrap();
                        self.emit_source_line(line, line_index)?;
                        self.emit_singleline_label(line, label)?;

                        self.emit_singleline_labels_in_current(line)?;
                    }
                    Either::Right(label) => {
                        let line_index = label.line_range.start;
                        self.current_line = line_index;

                        self.allocate_multiline_label(label);
                        self.start_new_multiline_labels();
                    }
                }
            } else {
                self.start_new_multiline_labels();

                let line = self.source.line(self.current_line).unwrap();
                self.emit_source_line(line, self.current_line)?;

                self.emit_singleline_labels_in_current(line)?;
                self.try_finishing_active_multiline_labels()?;

                self.current_line += 1;
            }
        }

        Ok(())
    }
}
