#[test]
fn form_schema_contracts_compile_at_the_type_boundary() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/vnext/pass/*.rs");
}
