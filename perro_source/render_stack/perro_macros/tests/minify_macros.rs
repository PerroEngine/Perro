const STRIPPED: &str = perro_macros::include_str_stripped!("tests/fixtures/minify_source.wgsl");
const MINIFIED: &str = perro_macros::include_min_str!("tests/fixtures/minify_source.wgsl");
const WGSL: &str = perro_macros::minified_wgsl!("tests/fixtures/minify_source.wgsl");
const CONCATENATED: &str = perro_macros::minified_wgsl!(concat!(
    include_str!("tests/fixtures/minify_source.wgsl"),
    include_str!("tests/fixtures/minify_tail.wgsl"),
));

#[test]
fn minify_macros_strip_comments_and_blank_lines() {
    let expected =
        "let a = 1; let url = \"http://example.test\"; let text = \"keep // inside string\";";

    assert_eq!(STRIPPED, expected);
    assert_eq!(MINIFIED, expected);
    assert_eq!(WGSL, expected);
}

#[test]
fn minify_macro_concats_and_tracks_all_sources() {
    assert_eq!(
        CONCATENATED,
        "let a = 1; let url = \"http://example.test\"; let text = \"keep // inside string\"; let b = 2;"
    );
}
