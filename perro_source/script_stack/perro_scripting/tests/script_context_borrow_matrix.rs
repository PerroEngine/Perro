#[test]
fn script_context_borrow_matrix() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/script_ctx_pass_prebind.rs");
    t.pass("tests/ui/script_ctx_pass_outside_closure.rs");
    t.compile_fail("tests/ui/script_ctx_fail_capture_ctx_in_run_closure.rs");
}
