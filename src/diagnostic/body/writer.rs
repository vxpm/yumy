use super::{BodyChunk, BodyDescriptor, Label};
use crate::Config;
use owo_colors::OwoColorize;
use std::io::Write;

#[derive(Debug, Clone)]
struct ActiveMultiline {
    label_id: usize,
    label: Label,
    recently_added: bool,
}

/// Struct responsible for writing a body described by a [`BodyDescriptor`].
pub(super) struct BodyWriter<'src, W> {
    writer: W,
    config: Config,
    descriptor: BodyDescriptor<'src>,
    slots: Vec<Option<ActiveMultiline>>,
    multiline_id: usize,
    line_number_width: usize,
    current_indent_level: usize,
}

impl<'src, W> BodyWriter<'src, W>
where
    W: Write,
{
    pub(super) fn new(writer: W, config: Config, descriptor: BodyDescriptor<'src>) -> Self {
        let slots_needed = descriptor.maximum_parallel_labels;
        let line_number_width = descriptor.line_number_width;

        Self {
            writer,
            config,
            descriptor,
            slots: vec![None; slots_needed],
            multiline_id: 0,
            line_number_width,
            current_indent_level: 0,
        }
    }

    fn emit_left_column(&mut self, line_index: Option<usize>) -> std::io::Result<()> {
        if let Some(index) = line_index {
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
            let Some(slot) = slot else {
                write!(self.writer, "  ")?;
                continue;
            };

            let style = slot
                .label
                .indicator_style
                .unwrap_or(self.config.styles.multiline_indicator);

            let indicator_char = if slot.recently_added {
                self.config.charset.multiline_start
            } else if finishing_multiline_labels.contains(&slot.label_id) {
                self.config.charset.multiline_end
            } else {
                self.config.charset.vertical_bar
            };

            slot.recently_added = false;
            write!(self.writer, "{} ", indicator_char.style(style))?;
        }

        Ok(())
    }

    fn emit_source_line(&mut self, chunk: &BodyChunk) -> std::io::Result<()> {
        self.emit_left_column(Some(chunk.line.index()))?;
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
            chunk.line.text().style(self.config.styles.source),
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
            let underline_range = before_underline_range.end
                ..(label.span.end().min(line.text().len() as u32) - local_base) as usize;

            // compute widths
            let before_underline_width =
                crate::text::dislay_width(&line.text()[before_underline_range]);
            let underline_width = crate::text::dislay_width(&line.text()[underline_range]);

            // write label
            let before_underline = std::iter::repeat(' ').take(before_underline_width);
            let underline = std::iter::repeat(self.config.charset.underliner).take(underline_width);
            let before_label: String = before_underline.chain(underline).collect();
            let label_style = label
                .indicator_style
                .unwrap_or(self.config.styles.singleline_indicator);

            writeln!(
                self.writer,
                "{:l$}{} {}",
                "",
                before_label.style(label_style),
                label.message.style(label_style),
                l = self.current_indent_level,
            )?;
        }

        Ok(())
    }

    fn allocate_multiline(&mut self, label: Label) {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("has enough slots");

        *slot = Some(ActiveMultiline {
            label_id: self.multiline_id,
            label,
            recently_added: true,
        });
        self.multiline_id += 1;
    }

    fn start_multiline_labels(&mut self, chunk: &mut BodyChunk) -> std::io::Result<()> {
        let labels = std::mem::take(&mut chunk.starting_multiline_labels);
        for label in labels {
            self.allocate_multiline(label);
        }

        Ok(())
    }

    fn emit_multiline_label(&mut self, label_id: usize) -> std::io::Result<()> {
        self.emit_left_column(None)?;

        let mut finished_multiline = None;
        let mut slots_iter = self.slots.iter_mut();
        while let Some(slot) = slots_iter.next() {
            let Some(active) = slot else {
                write!(self.writer, "  ",)?;
                continue;
            };

            let style = active
                .label
                .indicator_style
                .unwrap_or(self.config.styles.multiline_indicator);

            if active.label_id == label_id {
                finished_multiline = std::mem::take(slot);
                write!(
                    self.writer,
                    "{}{}",
                    self.config.charset.connection_top_to_right.style(style),
                    self.config.charset.horizontal_bar.style(style)
                )?;
                break;
            }

            write!(
                self.writer,
                "{} ",
                self.config.charset.vertical_bar.style(style)
            )?;
        }

        let finished_label = finished_multiline.unwrap().label;
        let finished_style = finished_label
            .indicator_style
            .unwrap_or(self.config.styles.multiline_indicator);

        while let Some(slot) = slots_iter.next() {
            let Some(active) = slot else {
                write!(self.writer, "  ",)?;
                continue;
            };

            let style = active
                .label
                .indicator_style
                .unwrap_or(self.config.styles.multiline_indicator);

            write!(
                self.writer,
                "{}{}",
                self.config.charset.multiline_crossing.style(style),
                self.config.charset.horizontal_bar.style(finished_style)
            )?;
        }

        writeln!(
            self.writer,
            " {}",
            finished_label.message.style(finished_style)
        )?;
        Ok(())
    }

    fn finish_multiline_labels(&mut self, chunk: &mut BodyChunk) -> std::io::Result<()> {
        let labels = std::mem::take(&mut chunk.finishing_multiline_labels);
        for label_id in labels {
            self.emit_multiline_label(label_id)?;
        }

        Ok(())
    }

    pub(super) fn write(mut self) -> std::io::Result<()> {
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
