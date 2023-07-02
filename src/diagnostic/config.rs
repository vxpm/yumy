use owo_colors::Style;

/// The charset to use when rendering a diagnostic.
#[derive(Debug, Clone)]
pub struct Charset {
    /// A vertical bar.
    pub vertical_bar: char,
    /// An horizontal bar.
    pub horizontal_bar: char,
    /// The character used to underline the source
    /// in single-line labels.
    pub underliner: char,
    /// The character that's used instead of the vertical
    /// bar when not in a source line.
    pub separator: char,
    /// The character that connects the vertical bar
    /// to the connector in multiline labels.
    pub connection_top_to_right: char,
    /// The character for when a multiline label starts.
    pub multiline_start: char,
    /// The character for when a multiline label ends.
    pub multiline_end: char,
    /// The character for when two multiline labels cross.
    pub multiline_crossing: char,
}

impl Default for Charset {
    fn default() -> Self {
        Self {
            vertical_bar: '│',
            horizontal_bar: '╶',
            underliner: '^',
            separator: ':',
            connection_top_to_right: '╰',
            multiline_start: '┬',
            multiline_end: '┼',
            multiline_crossing: '┼',
        }
    }
}

/// Default styles to use for each part of a diagnostic.
#[derive(Debug, Clone)]
pub struct DefaultStyles {
    pub source_name: Style,
    pub source: Style,
    pub left_column: Style,
    pub multiline_indicator: Style,
    pub singleline_indicator: Style,
    pub footnote_indicator: Style,
}

impl Default for DefaultStyles {
    fn default() -> Self {
        Self {
            source_name: Style::new().white().bold(),
            source: Style::new().white(),
            left_column: Style::new().bright_blue().bold(),
            multiline_indicator: Style::new().yellow(),
            singleline_indicator: Style::new().yellow(),
            footnote_indicator: Style::new().bright_blue().bold(),
        }
    }
}

/// Configuration used to render a diagnostic.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub charset: Charset,
    pub styles: DefaultStyles,
}
