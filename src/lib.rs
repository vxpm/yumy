#![doc=include_str!("../README.md")]

/// Module for diagnostic related items.
pub mod diagnostic;
/// Module for source related items.
pub mod source;

#[cfg(test)]
pub(crate) mod test;

pub use owo_colors;

pub use diagnostic::Diagnostic;
pub use diagnostic::Label;

pub use diagnostic::config::Charset;
pub use diagnostic::config::Config;
pub use diagnostic::config::DefaultStyles;

pub use source::Source;
pub use source::SourceSpan;
