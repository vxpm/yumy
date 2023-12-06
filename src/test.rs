use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[inline(always)]
pub(crate) fn type_name_of_val<T>(_: T) -> &'static str {
    std::any::type_name::<T>()
}

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

/// Asserts a snapshot of the diagnostic.
///
/// A diagnostic has 2 snapshots, `ansi` and `clean`. `ansi` has ANSI information while
/// `clean` is pure text.
macro_rules! diagnostic_snapshot {
    ($diagnostic:expr) => {{
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

        // setup settings
        let mut settings = ::insta::Settings::clone_current();
        settings.set_snapshot_path(crate::test::snapshots_path());
        let _guard = settings.bind_to_scope();

        ::insta::assert_snapshot!(clean);
        ::insta::assert_debug_snapshot!(ansi);
    }};
}

pub(crate) use diagnostic_snapshot;
