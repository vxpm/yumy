use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub(crate) const TEXT_SAMPLE_1: &str = include_str!("../samples/sample1.txt");
pub(crate) const RUST_SAMPLE_1: &str = include_str!("../samples/sample2.rs");
pub(crate) const RUST_SAMPLE_2: &str = include_str!("../samples/sample3.rs");
pub(crate) const TEXT_SAMPLE_2: &str = include_str!("../samples/sample4.txt");

#[inline(always)]
pub(crate) fn snapshots_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();

    PATH.get_or_init(|| {
        let mut p = std::env::current_dir().unwrap();
        p.push("snapshots");
        p
    })
    .as_path()
}

macro_rules! setup_insta {
    () => {
        let mut settings = ::insta::Settings::clone_current();
        settings.set_snapshot_path(crate::test::snapshots_path());
        let _guard = settings.bind_to_scope();
    };
}

/// Asserts a snapshot of the diagnostic.
///
/// A diagnostic has 2 snapshots, `ansi` and `clean`. `ansi` has ANSI information while
/// `clean` is pure text.
macro_rules! diagnostic_snapshot {
    (@inner compact: $diagnostic:expr) => {{
        let mut buffer = Vec::new();

        $diagnostic
            .write_to_compact(&mut buffer, &Config::default())
            .unwrap();

        let string = String::from_utf8(buffer).unwrap();

        // extract styles and clear it up
        let (clean, ansi): (Vec<_>, Vec<_>) = ansi_str::get_blocks(&string)
            .map(|block| (block.text().to_string(), block))
            .unzip();
        let clean = clean.join("");

        crate::test::setup_insta!();
        ::insta::assert_snapshot!(clean);
        ::insta::assert_debug_snapshot!(ansi);
    }};
    (@inner: $diagnostic:expr) => {{
        let mut buffer = Vec::new();

        $diagnostic
            .write_to(&mut buffer, &Config::default())
            .unwrap();

        let string = String::from_utf8(buffer).unwrap();

        // extract styles and clear it up
        let (clean, ansi): (Vec<_>, Vec<_>) = ansi_str::get_blocks(&string)
            .map(|block| (block.text().to_string(), block))
            .unzip();
        let clean = clean.join("");

        crate::test::setup_insta!();
        ::insta::assert_snapshot!(clean);
        ::insta::assert_debug_snapshot!(ansi);
    }};
    ($diagnostic:expr) => {
        diagnostic_snapshot!(@inner: $diagnostic);
        diagnostic_snapshot!(@inner compact: $diagnostic);
    }
}

pub(crate) use diagnostic_snapshot;
pub(crate) use setup_insta;
