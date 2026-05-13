---
scope: valen-ast
severity: major
dimension: test_coverage
---

# valen-ast: Zero unit tests

## 概要

valen-ast crate にテストが0件。Span::merge の debug_assert がリリースで無効になるなど、エッジケースが未検証。

## 改善案

span.rs に Span::new, len, is_empty, merge, Display のテストを追加。

## 影響範囲

- crates/valen-ast/src/span.rs
