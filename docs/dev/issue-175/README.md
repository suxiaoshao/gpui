# Issue #175 form implementation archive

- Status: `Historical delivery`; form API plans are `Superseded`
- Tracking issue: [#175](https://github.com/suxiaoshao/gpui/issues/175)
- Implementation PR: [#176](https://github.com/suxiaoshao/gpui/pull/176)
- Implementation commit: `6351898874b727ae8155903645a2dbfcc1f0da54`
- Successor: [Issue #199 form redesign](../issue-199/README.md)

PR #176 delivered the previous typed form implementation while closing Issue #175. The original
form plans described the infrastructure as having no independent issue, but their actual delivery
and review happened in that PR. They are therefore archived under Issue #175 by delivery provenance.

These files preserve the old `FormStore`, per-call field accessor, and field-owned weak entity design
as historical implementation evidence. They are not the current API contract and must not be used as
the implementation plan for Issue #199.

## Archived owner plans

- [`gpui-form`](../../../crates/gpui-form/docs/dev/issue-175/README.md)
- [`gpui-form-macros`](../../../crates/gpui-form-macros/docs/dev/issue-175/README.md)
- [`gpui-form-gpui-component`](../../../crates/gpui-form-gpui-component/docs/dev/issue-175/README.md)
- [Jaco form migration](../../../app/jaco/docs/dev/issue-175/gpui-form-migration.md)
- [Jaco Issue #175 product documents](../../../app/jaco/docs/dev/issue-175/README.md)

## Archival rule

Old API names may remain in these `issue-175` documents because they record the delivered design.
Active source, public API documentation, examples, tests, and Issue #199 plans are governed by the
new plan and may not cite this archive as current guidance.
