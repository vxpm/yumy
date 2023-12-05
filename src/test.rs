use std::path::PathBuf;

pub fn snapshots_path() -> PathBuf {
    let mut p = std::env::current_dir().unwrap();
    p.push("snapshots");

    p
}

macro_rules! diagnostic_snapshot {
    ($diagnostic:expr) => {{
        let mut buffer = Vec::new();

        $diagnostic
            .write_to(&mut buffer, &Config::default())
            .unwrap();

        let string = String::from_utf8(buffer).unwrap();

        // extract styles and clear it up
        let (clear, blocks): (Vec<_>, Vec<_>) = ansi_str::get_blocks(&string)
            .map(|block| (block.text().to_string(), block))
            .unzip();
        let clear = clear.join("");

        let mut settings = ::insta::Settings::clone_current();
        settings.set_snapshot_path(crate::test::snapshots_path());
        let _guard = settings.bind_to_scope();

        fn f() {}

        fn type_name_of_val<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }

        let mut name = type_name_of_val(f).strip_suffix("::f").unwrap_or("");
        while let Some(rest) = name.strip_suffix("::{{closure}}") {
            name = rest;
        }

        ::insta::assert_snapshot!(format!("{name}-clear"), clear);
        ::insta::assert_debug_snapshot!(format!("{name}-blocks"), blocks);
    }};
}

pub(crate) use diagnostic_snapshot;
