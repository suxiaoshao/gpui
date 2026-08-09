# `gpui-form-macros` documentation

This documentation describes the current `#[derive(FormSchema)]` workflow. The
macro creates typed schema definitions; the `gpui-form` runtime uses those
definitions with a single `Form<M>` editing session.

## Start here

- [README](../README.md) is the shortest complete loop: a flat model,
  `Form::new`, `Entity<Form<M>>`, root field read/write, `prepare`, and
  conditional rebase after persistence.
- [Guide](guide.md) builds that loop into nested children, optional values,
  recursive items, enum cases, topology mutations, validators, and conditional
  rebase after persistence.
- [中文指南](guide.zh-CN.md) mirrors the guide in Chinese.
