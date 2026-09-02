//! Snapshot tests over every fixture in tests/cases/**/*.vala.
//! Each fixture is formatted, snapshotted, checked for idempotency
//! (format(format(x)) == format(x)); the token-stream safety check runs
//! inside format_source itself.

use valafmt::{format_source, Options};

#[test]
fn fixtures() {
    insta::glob!("cases/**/*.vala", |path| {
        let src = std::fs::read_to_string(path).unwrap();
        let opts = Options::default();
        let out = match format_source(&src, &opts) {
            Ok(o) => o,
            Err(e) => panic!("failed to format {}:\n{e}", path.display()),
        };
        let again = format_source(&out, &opts)
            .unwrap_or_else(|e| panic!("reformat of {} failed:\n{e}", path.display()));
        assert_eq!(out, again, "formatting {} is not idempotent", path.display());
        insta::assert_snapshot!(out);
    });
}
