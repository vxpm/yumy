# yumy
a diagnostics rendering crate. yumy aims to:
- be easy to use
- be independent (i.e. to not require deep integration)
- be customizable enough

# example output
the diagnostic below is lorem ipsum.

![example diagnostic](https://iili.io/HiW4QDv.png)

here's the same diagnostic in compact mode:

![example diagnostic in compact mode](https://iili.io/HiWPJEB.png)

and here's the code for this diagnostic:

```rust, ignore
let diagnostic = Diagnostic::new(format!("{}: you did something wrong", "error".red()))
    .with_source(Source::new(&src, None))
    .with_label(Label::styled(
        SourceSpan::new(154, 580),
        "this is wrong!".bright_red(),
        Style::new().bright_red(),
    ))
    .with_label(Label::styled(
        SourceSpan::new(337, 362),
        "thats a little sus".bright_red(),
        Style::new().bright_red(),
    ))
    .with_label(Label::new(
        SourceSpan::new(421, 500),
        "you almost got this part right though".yellow(),
    ))
    .with_footnote(Footnote::new(format!(
        "{}: maybe try doing it differently",
        "help".yellow()
    )))
    .with_footnote(Footnote::new(format!(
        "{}: this message doesn't actually help you :)",
        "note".bright_green()
    )));

diagnostic.eprint(&Config::default()).unwrap();
```