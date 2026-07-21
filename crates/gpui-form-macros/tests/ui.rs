#[test]
fn invalid_form_attributes_have_stable_diagnostics() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/fail/*.rs");
}
