#[test]
fn form_schema_grammar_has_stable_diagnostics() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/vnext/fail/*.rs");
}
