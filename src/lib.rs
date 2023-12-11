#![doc=include_str!("../README.md")]

/// Module for diagnostic related items.
pub mod diagnostic;
/// Module for source related items.
pub mod source;
/// Module for testing related utilities.
#[cfg(test)]
pub(crate) mod test;
/// Module for text related utilities.
pub(crate) mod text;

pub use owo_colors;

pub use diagnostic::Diagnostic;
pub use diagnostic::Label;

pub use diagnostic::config::Charset;
pub use diagnostic::config::Config;
pub use diagnostic::config::DefaultStyles;

pub use source::Source;
pub use source::SourceSpan;
