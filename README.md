# yumy
a diagnostics rendering crate. yumy aims to be easy to use, focusing on simplicity. 

# example output
the diagnostic below is just an example.

![example diagnostic](https://iili.io/J5uF4ol.png)

here's the same diagnostic printed in compact mode:

![example diagnostic in compact mode](https://iili.io/J7LIskv.png)

and here's the code for this diagnostic (it's a test in this crate!):

```rust, ignore
let src = Source::new(crate::test::RUST_SAMPLE_2, Some("src/main.rs"));
let diagnostic =
    Diagnostic::new("error[E0277]: `Rc<Mutex<i32>>` cannot be sent between threads safely".red())
        .with_label(Label::styled(
            247..260u32,
            "required by a bound introduced by this call",
            Style::new().yellow()
        ))
        .with_label(Label::styled(
            261..357u32,
            "`Rc<Mutex<i32>>` cannot be sent between threads safely",
            Style::new().red()
        ))
        .with_footnote("note: required because it's used within `{closure@src/main.rs:11:36: 11:43}`".green())
        .with_footnote("help: within `{closure@src/main.rs:11:36: 11:43}`, the trait `Send` is not implemented for `Rc<Mutex<i32>>`".blue())
        .with_source(src);

diagnostic.eprint(&Config::default()).unwrap();
diagnostic.eprint_compact(&Config::default()).unwrap();
```
